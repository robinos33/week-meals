//! Couche infrastructure de `shopping-list` : implémentations SQLx des ports
//! du domaine.
//!
//! - [`SqlxShoppingListRepository`] — la liste elle-même ;
//! - [`SqlxReferenceRepository`] — le référentiel d'ingrédients (global) ;
//! - [`SqlxPlannedIngredients`] — projection **en lecture seule** des
//!   ingrédients planifiés, au travers du calendrier et des recettes.
//!
//! Requêtes runtime (aucune macro vérifiée à la compilation) ; erreurs SQLx
//! traduites en [`RepositoryError`].

use chrono::NaiveDate;
use kernel::{HouseholdId, Quantity, RepositoryError, ShoppingItemId, Unit};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

use crate::domain::{
    CookedCountRecorder, IngredientReference, PlannedIngredient, PlannedIngredientsSource,
    ReferenceCatalog, ReferenceRepository, ShoppingItem, ShoppingListRepository,
};

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

/// Reconstruit une quantité depuis ses colonnes.
fn quantity(amount: f64, unit: &str) -> Result<Quantity, RepositoryError> {
    let unit = parse_unit(unit)?;
    Quantity::new(amount, unit)
        .map_err(|error| RepositoryError::Backend(format!("stored quantity invalid: {error}")))
}

// --- Liste de courses -----------------------------------------------------

/// Implémentation SQLx du [`ShoppingListRepository`].
pub struct SqlxShoppingListRepository {
    pool: SqlitePool,
}

impl SqlxShoppingListRepository {
    /// Construit le repository sur un pool partagé.
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

/// Colonnes lues pour reconstruire une ligne.
const ITEM_COLUMNS: &str = "id, name, amount, unit, category, checked, generated, position";

/// Reconstruit une ligne depuis un enregistrement.
fn item_from_row(
    row: &sqlx::sqlite::SqliteRow,
    household_id: HouseholdId,
) -> Result<ShoppingItem, RepositoryError> {
    let id: Uuid = row.try_get("id").map_err(backend)?;
    let amount: f64 = row.try_get("amount").map_err(backend)?;
    let unit: String = row.try_get("unit").map_err(backend)?;
    Ok(ShoppingItem {
        id: ShoppingItemId::from(id),
        household_id,
        name: row.try_get("name").map_err(backend)?,
        quantity: quantity(amount, &unit)?,
        category: row.try_get("category").map_err(backend)?,
        checked: row.try_get("checked").map_err(backend)?,
        generated: row.try_get("generated").map_err(backend)?,
        position: row.try_get("position").map_err(backend)?,
    })
}

#[async_trait::async_trait]
impl ShoppingListRepository for SqlxShoppingListRepository {
    async fn list(&self, household_id: HouseholdId) -> Result<Vec<ShoppingItem>, RepositoryError> {
        // `position` porte l'ordre d'affichage (réordonnable par glisser-déposer) ;
        // `created_at` départage d'éventuelles positions égales.
        let rows = sqlx::query(&format!(
            "select {ITEM_COLUMNS} from shopping_list_items \
             where household_id = ? \
             order by position, created_at"
        ))
        .bind(household_id.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(backend)?;

        rows.iter()
            .map(|row| item_from_row(row, household_id))
            .collect()
    }

    async fn find(
        &self,
        household_id: HouseholdId,
        id: ShoppingItemId,
    ) -> Result<Option<ShoppingItem>, RepositoryError> {
        let row = sqlx::query(&format!(
            "select {ITEM_COLUMNS} from shopping_list_items where id = ? and household_id = ?"
        ))
        .bind(id.as_uuid())
        .bind(household_id.as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(backend)?;

        row.map(|row| item_from_row(&row, household_id)).transpose()
    }

    async fn replace_generated(
        &self,
        household_id: HouseholdId,
        items: &[ShoppingItem],
    ) -> Result<(), RepositoryError> {
        // En transaction : la liste ne doit jamais être observée à moitié
        // régénérée.
        let mut tx = self.pool.begin().await.map_err(backend)?;

        sqlx::query("delete from shopping_list_items where household_id = ? and generated = 1")
            .bind(household_id.as_uuid())
            .execute(&mut *tx)
            .await
            .map_err(backend)?;

        // Les lignes générées occupent les positions `0..n` ; on décale les
        // ajouts manuels restants pour qu'ils s'affichent en dessous, sans
        // collision (leur ordre relatif est préservé).
        let shift = i32::try_from(items.len()).unwrap_or(i32::MAX);
        sqlx::query(
            "update shopping_list_items set position = position + ? where household_id = ?",
        )
        .bind(shift)
        .bind(household_id.as_uuid())
        .execute(&mut *tx)
        .await
        .map_err(backend)?;

        for (index, item) in items.iter().enumerate() {
            let position = i32::try_from(index).unwrap_or(i32::MAX);
            sqlx::query(
                "insert into shopping_list_items \
                 (id, household_id, name, amount, unit, category, checked, generated, position) \
                 values (?, ?, ?, ?, ?, ?, ?, 1, ?)",
            )
            .bind(item.id.as_uuid())
            .bind(household_id.as_uuid())
            .bind(&item.name)
            .bind(item.quantity.amount())
            .bind(item.quantity.unit().as_str())
            .bind(item.category.as_deref())
            .bind(item.checked)
            .bind(position)
            .execute(&mut *tx)
            .await
            .map_err(backend)?;
        }

        tx.commit().await.map_err(backend)
    }

    async fn add(&self, item: &ShoppingItem) -> Result<(), RepositoryError> {
        // Position calculée en base : la ligne s'ajoute en fin de liste.
        sqlx::query(
            "insert into shopping_list_items \
             (id, household_id, name, amount, unit, category, checked, generated, position) \
             values (?, ?, ?, ?, ?, ?, ?, ?, \
               coalesce((select max(position) + 1 from shopping_list_items where household_id = ?), 0))",
        )
        .bind(item.id.as_uuid())
        .bind(item.household_id.as_uuid())
        .bind(&item.name)
        .bind(item.quantity.amount())
        .bind(item.quantity.unit().as_str())
        .bind(item.category.as_deref())
        .bind(item.checked)
        .bind(item.generated)
        // Le foyer est lié une seconde fois : les `?` de SQLite sont
        // positionnels, un même paramètre ne se réutilise pas comme `$2`.
        .bind(item.household_id.as_uuid())
        .execute(&self.pool)
        .await
        .map_err(backend)?;
        Ok(())
    }

    async fn reorder(
        &self,
        household_id: HouseholdId,
        ordered_ids: &[ShoppingItemId],
    ) -> Result<(), RepositoryError> {
        // En transaction : l'ordre ne doit jamais être observé à moitié appliqué.
        // Le filtre `household_id` empêche de bouger une ligne d'un autre foyer.
        let mut tx = self.pool.begin().await.map_err(backend)?;
        for (index, id) in ordered_ids.iter().enumerate() {
            sqlx::query(
                "update shopping_list_items set position = ? where id = ? and household_id = ?",
            )
            .bind(i32::try_from(index).unwrap_or(i32::MAX))
            .bind(id.as_uuid())
            .bind(household_id.as_uuid())
            .execute(&mut *tx)
            .await
            .map_err(backend)?;
        }
        tx.commit().await.map_err(backend)
    }

    async fn update(&self, item: &ShoppingItem) -> Result<(), RepositoryError> {
        let result = sqlx::query(
            "update shopping_list_items \
             set name = ?, amount = ?, unit = ?, category = ?, checked = ? \
             where id = ? and household_id = ?",
        )
        .bind(&item.name)
        .bind(item.quantity.amount())
        .bind(item.quantity.unit().as_str())
        .bind(item.category.as_deref())
        .bind(item.checked)
        .bind(item.id.as_uuid())
        .bind(item.household_id.as_uuid())
        .execute(&self.pool)
        .await
        .map_err(backend)?;

        if result.rows_affected() == 0 {
            Err(RepositoryError::NotFound)
        } else {
            Ok(())
        }
    }

    async fn delete(
        &self,
        household_id: HouseholdId,
        id: ShoppingItemId,
    ) -> Result<(), RepositoryError> {
        let result =
            sqlx::query("delete from shopping_list_items where id = ? and household_id = ?")
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

    async fn clear_checked(&self, household_id: HouseholdId) -> Result<u64, RepositoryError> {
        let result =
            sqlx::query("delete from shopping_list_items where household_id = ? and checked = 1")
                .bind(household_id.as_uuid())
                .execute(&self.pool)
                .await
                .map_err(backend)?;
        Ok(result.rows_affected())
    }
}

// --- Référentiel ----------------------------------------------------------

/// Implémentation SQLx du [`ReferenceRepository`].
pub struct SqlxReferenceRepository {
    pool: SqlitePool,
}

impl SqlxReferenceRepository {
    /// Construit le repository sur un pool partagé.
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insère ou met à jour des références (seed depuis `data/ingredients.yaml`).
    ///
    /// Le nom est **normalisé** avant écriture : c'est la clé de rapprochement
    /// du service de conversion. Upsert par nom ⇒ rejouer le seed est sans
    /// effet de bord. Renvoie le nombre de références écrites.
    ///
    /// # Errors
    /// [`RepositoryError::Backend`] sur panne technique.
    pub async fn upsert_all(
        &self,
        references: &[IngredientReference],
    ) -> Result<u64, RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(backend)?;
        let mut written = 0;
        for reference in references {
            sqlx::query(
                "insert into ingredient_reference (name, category, avg_weight_g, countable) \
                 values (?, ?, ?, ?) \
                 on conflict (name) do update set \
                   category = excluded.category, \
                   avg_weight_g = excluded.avg_weight_g, \
                   countable = excluded.countable, \
                   updated_at = datetime('now')",
            )
            .bind(crate::domain::reference::normalize_name(&reference.name))
            .bind(&reference.category)
            .bind(i32::try_from(reference.avg_weight_g).unwrap_or(i32::MAX))
            .bind(reference.countable)
            .execute(&mut *tx)
            .await
            .map_err(backend)?;
            written += 1;
        }
        tx.commit().await.map_err(backend)?;
        Ok(written)
    }
}

#[async_trait::async_trait]
impl ReferenceRepository for SqlxReferenceRepository {
    async fn catalog(&self) -> Result<ReferenceCatalog, RepositoryError> {
        let rows =
            sqlx::query("select name, category, avg_weight_g, countable from ingredient_reference")
                .fetch_all(&self.pool)
                .await
                .map_err(backend)?;

        rows.iter()
            .map(|row| {
                let name: String = row.try_get("name").map_err(backend)?;
                let category: String = row.try_get("category").map_err(backend)?;
                let avg_weight_g: i32 = row.try_get("avg_weight_g").map_err(backend)?;
                let countable: bool = row.try_get("countable").map_err(backend)?;
                let avg_weight_g = u32::try_from(avg_weight_g).map_err(|_| {
                    RepositoryError::Backend(format!("negative avg_weight_g for {name}"))
                })?;
                Ok(IngredientReference::new(
                    name,
                    category,
                    avg_weight_g,
                    countable,
                ))
            })
            .collect::<Result<Vec<_>, RepositoryError>>()
            .map(ReferenceCatalog::from_iter)
    }
}

// --- Ingrédients planifiés ------------------------------------------------

/// Projection SQL des ingrédients planifiés (calendrier × recettes).
///
/// Lit des tables appartenant à d'autres domaines (`meal_plan`,
/// `recipe_ingredients`) : c'est un choix **délibéré et confiné à
/// l'infrastructure**, qui évite un N+1 (une requête au lieu d'une par recette
/// planifiée). Le domaine, lui, ne voit que le port
/// [`PlannedIngredientsSource`].
pub struct SqlxPlannedIngredients {
    pool: SqlitePool,
}

impl SqlxPlannedIngredients {
    /// Construit la projection sur un pool partagé.
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl PlannedIngredientsSource for SqlxPlannedIngredients {
    async fn planned(
        &self,
        household_id: HouseholdId,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<PlannedIngredient>, RepositoryError> {
        // Une recette planifiée deux fois dans la plage produit deux jeux de
        // lignes : l'agrégation (domaine) cumulera les quantités.
        let rows = sqlx::query(
            "select ri.name, ri.quantity, ri.unit \
             from meal_plan mp \
             join recipe_ingredients ri on ri.recipe_id = mp.recipe_id \
             where mp.household_id = ? and mp.meal_date between ? and ? \
             order by mp.meal_date, mp.slot, ri.position",
        )
        .bind(household_id.as_uuid())
        .bind(from)
        .bind(to)
        .fetch_all(&self.pool)
        .await
        .map_err(backend)?;

        rows.iter()
            .map(|row| {
                let name: String = row.try_get("name").map_err(backend)?;
                let amount: f64 = row.try_get("quantity").map_err(backend)?;
                let unit: String = row.try_get("unit").map_err(backend)?;
                Ok(PlannedIngredient::new(name, quantity(amount, &unit)?))
            })
            .collect()
    }
}

// --- Compteur « cuisiné X fois » ------------------------------------------

/// Enregistre les recettes cuisinées (#58), en écrivant sur `meal_plan` et
/// `recipes`. Comme [`SqlxPlannedIngredients`], il croise volontairement
/// d'autres domaines, confiné à l'infrastructure.
pub struct SqlxCookedCounter {
    pool: SqlitePool,
}

impl SqlxCookedCounter {
    /// Construit le compteur sur un pool partagé.
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl CookedCountRecorder for SqlxCookedCounter {
    async fn record_cooked(
        &self,
        household_id: HouseholdId,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<(), RepositoryError> {
        // SQLite n'autorise pas d'`update` dans un `with` : là où Postgres
        // tenait en un seul énoncé (CTE modificatrice), il en faut deux, dans
        // une transaction pour rester atomique.
        //
        // L'ordre porte la correction : on incrémente **d'abord** les
        // compteurs, tant que les créneaux à compter sont encore reconnaissables
        // à leur `counted_at is null` ; on les marque ensuite. L'inverse
        // n'incrémenterait plus rien.
        let mut tx = self.pool.begin().await.map_err(backend)?;

        // Chaque recette du foyer gagne autant que de créneaux vierges où elle
        // apparaît dans la plage — posée sur deux créneaux, elle compte deux
        // fois. `last_cooked_at` départage le podium à égalité.
        sqlx::query(
            "update recipes set \
                 cooked_count = cooked_count + ( \
                     select count(*) from meal_plan mp \
                     where mp.recipe_id = recipes.id and mp.household_id = ? \
                       and mp.meal_date between ? and ? and mp.counted_at is null \
                 ), \
                 last_cooked_at = datetime('now') \
             where household_id = ? and exists ( \
                 select 1 from meal_plan mp \
                 where mp.recipe_id = recipes.id and mp.household_id = ? \
                   and mp.meal_date between ? and ? and mp.counted_at is null \
             )",
        )
        .bind(household_id.as_uuid())
        .bind(from)
        .bind(to)
        .bind(household_id.as_uuid())
        .bind(household_id.as_uuid())
        .bind(from)
        .bind(to)
        .execute(&mut *tx)
        .await
        .map_err(backend)?;

        sqlx::query(
            "update meal_plan set counted_at = datetime('now') \
             where household_id = ? and meal_date between ? and ? and counted_at is null",
        )
        .bind(household_id.as_uuid())
        .bind(from)
        .bind(to)
        .execute(&mut *tx)
        .await
        .map_err(backend)?;

        tx.commit().await.map_err(backend)
    }
}
