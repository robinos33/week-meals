//! Couche infrastructure de `recipes` : implémentation SQLx du
//! [`RecipeRepository`](crate::domain::RecipeRepository).
//!
//! Une recette est répartie sur trois tables (`recipes`, `recipe_ingredients`,
//! `recipe_steps`) : les écritures se font en **transaction** et les enfants
//! sont réécrits en bloc à chaque mise à jour. Requêtes runtime (aucune macro
//! vérifiée à la compilation) ; erreurs SQLx traduites en [`RepositoryError`].

use std::collections::HashMap;

use kernel::{HouseholdId, Quantity, RecipeId, RepositoryError, Unit};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::domain::{Recipe, RecipeIngredient, RecipeRepository};

/// Traduit une erreur SQLx en erreur de repository agnostique.
fn backend(err: sqlx::Error) -> RepositoryError {
    RepositoryError::Backend(err.to_string())
}

/// Interprète l'unité stockée (texte canonique du `kernel`).
fn parse_unit(raw: &str) -> Result<Unit, RepositoryError> {
    match raw {
        "g" => Ok(Unit::G),
        "kg" => Ok(Unit::Kg),
        "ml" => Ok(Unit::Ml),
        "l" => Ok(Unit::L),
        "piece" => Ok(Unit::Piece),
        other => Err(RepositoryError::Backend(format!(
            "unknown stored unit: {other}"
        ))),
    }
}

/// Convertit un entier SQL signé (contraint `>= 0`) en minutes optionnelles.
fn minutes(value: Option<i32>) -> Option<u32> {
    value.and_then(|v| u32::try_from(v).ok())
}

#[derive(sqlx::FromRow)]
struct RecipeRow {
    id: Uuid,
    household_id: Uuid,
    title: String,
    photo: Option<String>,
    prep_time_min: Option<i32>,
    cook_time_min: Option<i32>,
}

#[derive(sqlx::FromRow)]
struct IngredientRow {
    recipe_id: Uuid,
    name: String,
    quantity: f64,
    unit: String,
}

#[derive(sqlx::FromRow)]
struct StepRow {
    recipe_id: Uuid,
    instruction: String,
}

impl IngredientRow {
    fn into_domain(self) -> Result<RecipeIngredient, RepositoryError> {
        let unit = parse_unit(&self.unit)?;
        let quantity = Quantity::new(self.quantity, unit)
            .map_err(|e| RepositoryError::Backend(format!("invalid stored quantity: {e}")))?;
        RecipeIngredient::new(self.name, quantity)
            .map_err(|e| RepositoryError::Backend(format!("invalid stored ingredient: {e}")))
    }
}

/// Assemble une recette depuis sa ligne principale et ses enfants ordonnés.
fn assemble(
    row: RecipeRow,
    ingredients: Vec<RecipeIngredient>,
    steps: Vec<String>,
) -> Result<Recipe, RepositoryError> {
    Recipe::from_parts(
        RecipeId::from(row.id),
        HouseholdId::from(row.household_id),
        row.title,
        minutes(row.prep_time_min),
        minutes(row.cook_time_min),
        row.photo,
        ingredients,
        steps,
    )
    .map_err(|e| RepositoryError::Backend(format!("invalid stored recipe: {e}")))
}

/// Repository SQLx des recettes.
#[derive(Clone)]
pub struct SqlxRecipeRepository {
    pool: PgPool,
}

impl SqlxRecipeRepository {
    /// Construit le repository à partir d'un pool Postgres.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Insère les ingrédients et étapes d'une recette dans la transaction.
    async fn insert_children(
        tx: &mut Transaction<'_, Postgres>,
        recipe: &Recipe,
    ) -> Result<(), RepositoryError> {
        for (position, ingredient) in recipe.ingredients.iter().enumerate() {
            sqlx::query(
                "insert into recipe_ingredients (recipe_id, position, name, quantity, unit) \
                 values ($1, $2, $3, $4, $5)",
            )
            .bind(recipe.id.as_uuid())
            .bind(i32::try_from(position).unwrap_or(i32::MAX))
            .bind(&ingredient.name)
            .bind(ingredient.quantity.amount())
            .bind(ingredient.quantity.unit().as_str())
            .execute(&mut **tx)
            .await
            .map_err(backend)?;
        }
        for (position, step) in recipe.steps.iter().enumerate() {
            sqlx::query(
                "insert into recipe_steps (recipe_id, position, instruction) values ($1, $2, $3)",
            )
            .bind(recipe.id.as_uuid())
            .bind(i32::try_from(position).unwrap_or(i32::MAX))
            .bind(step)
            .execute(&mut **tx)
            .await
            .map_err(backend)?;
        }
        Ok(())
    }

    /// Charge les ingrédients (ordonnés) d'un ensemble de recettes, groupés par
    /// identifiant de recette.
    async fn load_ingredients(
        &self,
        recipe_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, Vec<RecipeIngredient>>, RepositoryError> {
        let rows: Vec<IngredientRow> = sqlx::query_as(
            "select recipe_id, name, quantity, unit from recipe_ingredients \
             where recipe_id = any($1) order by recipe_id, position",
        )
        .bind(recipe_ids)
        .fetch_all(&self.pool)
        .await
        .map_err(backend)?;

        let mut grouped: HashMap<Uuid, Vec<RecipeIngredient>> = HashMap::new();
        for row in rows {
            let recipe_id = row.recipe_id;
            grouped
                .entry(recipe_id)
                .or_default()
                .push(row.into_domain()?);
        }
        Ok(grouped)
    }

    /// Charge les étapes (ordonnées) d'un ensemble de recettes, groupées par
    /// identifiant de recette.
    async fn load_steps(
        &self,
        recipe_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, Vec<String>>, RepositoryError> {
        let rows: Vec<StepRow> = sqlx::query_as(
            "select recipe_id, instruction from recipe_steps \
             where recipe_id = any($1) order by recipe_id, position",
        )
        .bind(recipe_ids)
        .fetch_all(&self.pool)
        .await
        .map_err(backend)?;

        let mut grouped: HashMap<Uuid, Vec<String>> = HashMap::new();
        for row in rows {
            grouped
                .entry(row.recipe_id)
                .or_default()
                .push(row.instruction);
        }
        Ok(grouped)
    }

    /// Assemble une liste de recettes depuis leurs lignes principales,
    /// en chargeant leurs enfants en deux requêtes groupées.
    async fn assemble_all(&self, rows: Vec<RecipeRow>) -> Result<Vec<Recipe>, RepositoryError> {
        let ids: Vec<Uuid> = rows.iter().map(|r| r.id).collect();
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut ingredients = self.load_ingredients(&ids).await?;
        let mut steps = self.load_steps(&ids).await?;

        rows.into_iter()
            .map(|row| {
                let id = row.id;
                assemble(
                    row,
                    ingredients.remove(&id).unwrap_or_default(),
                    steps.remove(&id).unwrap_or_default(),
                )
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl RecipeRepository for SqlxRecipeRepository {
    async fn create(&self, recipe: &Recipe) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(backend)?;
        sqlx::query(
            "insert into recipes (id, household_id, title, photo, prep_time_min, cook_time_min) \
             values ($1, $2, $3, $4, $5, $6)",
        )
        .bind(recipe.id.as_uuid())
        .bind(recipe.household_id.as_uuid())
        .bind(&recipe.title)
        .bind(recipe.photo.as_deref())
        .bind(recipe.prep_time_min.map(|m| m as i32))
        .bind(recipe.cook_time_min.map(|m| m as i32))
        .execute(&mut *tx)
        .await
        .map_err(backend)?;
        Self::insert_children(&mut tx, recipe).await?;
        tx.commit().await.map_err(backend)
    }

    async fn find(
        &self,
        household_id: HouseholdId,
        id: RecipeId,
    ) -> Result<Option<Recipe>, RepositoryError> {
        let row: Option<RecipeRow> = sqlx::query_as(
            "select id, household_id, title, photo, prep_time_min, cook_time_min \
             from recipes where id = $1 and household_id = $2",
        )
        .bind(id.as_uuid())
        .bind(household_id.as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(backend)?;

        match row {
            None => Ok(None),
            Some(row) => Ok(self.assemble_all(vec![row]).await?.into_iter().next()),
        }
    }

    async fn list(&self, household_id: HouseholdId) -> Result<Vec<Recipe>, RepositoryError> {
        let rows: Vec<RecipeRow> = sqlx::query_as(
            "select id, household_id, title, photo, prep_time_min, cook_time_min \
             from recipes where household_id = $1 order by lower(title)",
        )
        .bind(household_id.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(backend)?;
        self.assemble_all(rows).await
    }

    async fn search(
        &self,
        household_id: HouseholdId,
        query: &str,
    ) -> Result<Vec<Recipe>, RepositoryError> {
        let pattern = format!("%{}%", query.trim());
        let rows: Vec<RecipeRow> = sqlx::query_as(
            "select id, household_id, title, photo, prep_time_min, cook_time_min \
             from recipes where household_id = $1 and title ilike $2 order by lower(title)",
        )
        .bind(household_id.as_uuid())
        .bind(pattern)
        .fetch_all(&self.pool)
        .await
        .map_err(backend)?;
        self.assemble_all(rows).await
    }

    async fn update(&self, recipe: &Recipe) -> Result<(), RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(backend)?;
        let result = sqlx::query(
            "update recipes set title = $3, photo = $4, prep_time_min = $5, \
             cook_time_min = $6, updated_at = now() where id = $1 and household_id = $2",
        )
        .bind(recipe.id.as_uuid())
        .bind(recipe.household_id.as_uuid())
        .bind(&recipe.title)
        .bind(recipe.photo.as_deref())
        .bind(recipe.prep_time_min.map(|m| m as i32))
        .bind(recipe.cook_time_min.map(|m| m as i32))
        .execute(&mut *tx)
        .await
        .map_err(backend)?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        // Remplacement complet des enfants.
        sqlx::query("delete from recipe_ingredients where recipe_id = $1")
            .bind(recipe.id.as_uuid())
            .execute(&mut *tx)
            .await
            .map_err(backend)?;
        sqlx::query("delete from recipe_steps where recipe_id = $1")
            .bind(recipe.id.as_uuid())
            .execute(&mut *tx)
            .await
            .map_err(backend)?;
        Self::insert_children(&mut tx, recipe).await?;

        tx.commit().await.map_err(backend)
    }

    async fn delete(&self, household_id: HouseholdId, id: RecipeId) -> Result<(), RepositoryError> {
        // Les enfants partent en cascade (FK on delete cascade).
        let result = sqlx::query("delete from recipes where id = $1 and household_id = $2")
            .bind(id.as_uuid())
            .bind(household_id.as_uuid())
            .execute(&self.pool)
            .await
            .map_err(backend)?;
        if result.rows_affected() == 0 {
            Err(RepositoryError::NotFound)
        } else {
            Ok(())
        }
    }
}
