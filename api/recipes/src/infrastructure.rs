//! Couche infrastructure de `recipes` : implémentation SQLx du
//! [`RecipeRepository`](crate::domain::RecipeRepository).
//!
//! Une recette est répartie sur trois tables (`recipes`, `recipe_ingredients`,
//! `recipe_steps`) : les écritures se font en **transaction** et les enfants
//! sont réécrits en bloc à chaque mise à jour. Requêtes runtime (aucune macro
//! vérifiée à la compilation) ; erreurs SQLx traduites en [`RepositoryError`].

use std::collections::HashMap;

use kernel::{HouseholdId, Quantity, RecipeId, RepositoryError, Unit};
use sqlx::{Sqlite, SqlitePool, Transaction};
use uuid::Uuid;

use crate::domain::{normalize_title, Recipe, RecipeIngredient, RecipeRepository};

/// Import d'une recette par URL (scraping JSON-LD + garde SSRF).
pub mod scrape;
pub use scrape::HttpRecipeScraper;

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

/// Convertit des minutes du domaine en entier SQL. Le domaine borne les temps à
/// [`MAX_TIME_MIN`] (cf. [`Recipe::from_parts`]) : la saturation est donc
/// inatteignable, elle évite juste un `as` qui replierait la valeur en négatif.
fn minutes_to_sql(value: Option<u32>) -> Option<i32> {
    value.map(|m| i32::try_from(m).unwrap_or(i32::MAX))
}

/// Échappe les jokers `LIKE` (`%`, `_`) et le caractère d'échappement lui-même,
/// pour qu'une recherche sur « 100% » cherche bien « 100% » et non « 100 » suivi
/// de n'importe quoi. À utiliser avec `like ... escape '\'`.
fn escape_like(input: &str) -> String {
    input
        .replace('\\', r"\\")
        .replace('%', r"\%")
        .replace('_', r"\_")
}

/// Liste de `?` séparés par des virgules, pour un `in (…)`.
///
/// SQLite ne sait pas lier un tableau à un seul paramètre : le `= any($1)` de
/// Postgres n'a pas d'équivalent, il faut autant de marqueurs que de valeurs
/// (cf. ADR-0008). Les identifiants viennent d'une requête précédente, jamais
/// de l'utilisateur, et restent liés un par un — la chaîne construite ici ne
/// contient que des `?`.
fn placeholders(count: usize) -> String {
    vec!["?"; count].join(", ")
}

#[derive(sqlx::FromRow)]
struct RecipeRow {
    id: Uuid,
    household_id: Uuid,
    title: String,
    photo: Option<String>,
    prep_time_min: Option<i32>,
    cook_time_min: Option<i32>,
    cooked_count: i32,
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
    // `cooked_count` est contraint `>= 0` en base ; un négatif serait une
    // corruption — on retombe sur `0` plutôt que de paniquer.
    let cooked_count = u32::try_from(row.cooked_count).unwrap_or(0);
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
    .map(|recipe| recipe.with_cooked_count(cooked_count))
    .map_err(|e| RepositoryError::Backend(format!("invalid stored recipe: {e}")))
}

/// Repository SQLx des recettes.
#[derive(Clone)]
pub struct SqlxRecipeRepository {
    pool: SqlitePool,
}

impl SqlxRecipeRepository {
    /// Construit le repository à partir d'un pool SQLite.
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insère les ingrédients et étapes d'une recette dans la transaction.
    async fn insert_children(
        tx: &mut Transaction<'_, Sqlite>,
        recipe: &Recipe,
    ) -> Result<(), RepositoryError> {
        for (position, ingredient) in recipe.ingredients.iter().enumerate() {
            sqlx::query(
                "insert into recipe_ingredients (recipe_id, position, name, quantity, unit) \
                 values (?, ?, ?, ?, ?)",
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
                "insert into recipe_steps (recipe_id, position, instruction) values (?, ?, ?)",
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
        let sql = format!(
            "select recipe_id, name, quantity, unit from recipe_ingredients \
             where recipe_id in ({}) order by recipe_id, position",
            placeholders(recipe_ids.len())
        );
        let mut query = sqlx::query_as(&sql);
        for id in recipe_ids {
            query = query.bind(*id);
        }
        let rows: Vec<IngredientRow> = query.fetch_all(&self.pool).await.map_err(backend)?;

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
        let sql = format!(
            "select recipe_id, instruction from recipe_steps \
             where recipe_id in ({}) order by recipe_id, position",
            placeholders(recipe_ids.len())
        );
        let mut query = sqlx::query_as(&sql);
        for id in recipe_ids {
            query = query.bind(*id);
        }
        let rows: Vec<StepRow> = query.fetch_all(&self.pool).await.map_err(backend)?;

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
            "insert into recipes \
             (id, household_id, title, title_norm, photo, prep_time_min, cook_time_min) \
             values (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(recipe.id.as_uuid())
        .bind(recipe.household_id.as_uuid())
        .bind(&recipe.title)
        .bind(normalize_title(&recipe.title))
        .bind(recipe.photo.as_deref())
        .bind(minutes_to_sql(recipe.prep_time_min))
        .bind(minutes_to_sql(recipe.cook_time_min))
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
            "select id, household_id, title, photo, prep_time_min, cook_time_min, cooked_count \
             from recipes where id = ? and household_id = ?",
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
            "select id, household_id, title, photo, prep_time_min, cook_time_min, cooked_count \
             from recipes where household_id = ? order by title_norm",
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
        // Recherche sur la clé normalisée des deux côtés : « CRÈME » comme
        // « creme » trouvent « Crème brûlée » (cf. ADR-0008).
        let pattern = format!("%{}%", escape_like(&normalize_title(query)));
        let rows: Vec<RecipeRow> = sqlx::query_as(
            "select id, household_id, title, photo, prep_time_min, cook_time_min, cooked_count \
             from recipes where household_id = ? and title_norm like ? escape '\\' \
             order by title_norm",
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
        // Marqueurs positionnels (`?`) : contrairement aux `$n` de Postgres,
        // l'ordre des `bind` suit celui du texte SQL — le `set` d'abord, le
        // `where` ensuite.
        let result = sqlx::query(
            "update recipes set title = ?, title_norm = ?, photo = ?, prep_time_min = ?, \
             cook_time_min = ?, updated_at = datetime('now') \
             where id = ? and household_id = ?",
        )
        .bind(&recipe.title)
        .bind(normalize_title(&recipe.title))
        .bind(recipe.photo.as_deref())
        .bind(minutes_to_sql(recipe.prep_time_min))
        .bind(minutes_to_sql(recipe.cook_time_min))
        .bind(recipe.id.as_uuid())
        .bind(recipe.household_id.as_uuid())
        .execute(&mut *tx)
        .await
        .map_err(backend)?;

        if result.rows_affected() == 0 {
            return Err(RepositoryError::NotFound);
        }

        // Remplacement complet des enfants.
        sqlx::query("delete from recipe_ingredients where recipe_id = ?")
            .bind(recipe.id.as_uuid())
            .execute(&mut *tx)
            .await
            .map_err(backend)?;
        sqlx::query("delete from recipe_steps where recipe_id = ?")
            .bind(recipe.id.as_uuid())
            .execute(&mut *tx)
            .await
            .map_err(backend)?;
        Self::insert_children(&mut tx, recipe).await?;

        tx.commit().await.map_err(backend)
    }

    async fn delete(&self, household_id: HouseholdId, id: RecipeId) -> Result<(), RepositoryError> {
        // Les enfants partent en cascade (FK on delete cascade).
        let result = sqlx::query("delete from recipes where id = ? and household_id = ?")
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

// --- Stockage des photos (S3-compatible : Cloudflare R2 / MinIO) -------------

use s3::creds::Credentials;
use s3::{Bucket, Region};

use crate::domain::{photo_extension, PhotoError, PhotoStorage, PhotoUpload};

/// Configuration du stockage photo (lue depuis l'environnement par le `server`).
#[derive(Debug, Clone)]
pub struct R2Config {
    /// Endpoint S3 : URL du compte R2 en prod, `http://localhost:9000` (MinIO) en dev.
    pub endpoint: String,
    /// Région S3. `auto` pour R2, `us-east-1` pour MinIO.
    pub region: String,
    /// Nom du bucket.
    pub bucket: String,
    /// Clé d'accès.
    pub access_key: String,
    /// Clé secrète.
    pub secret_key: String,
    /// Base publique des URLs stockées (domaine public R2, ou l'endpoint MinIO).
    pub public_base_url: String,
    /// Durée de validité d'une URL présignée, en secondes.
    pub expiry_secs: u32,
}

/// Implémentation S3 du port [`PhotoStorage`], via `rust-s3`.
///
/// Utilise le **path-style** (`endpoint/bucket/clé`) pour rester compatible avec
/// un endpoint custom (R2, MinIO) sans DNS par bucket. La présignature est
/// hors-ligne : aucun octet de fichier ne passe par l'API.
pub struct R2PhotoStorage {
    bucket: Box<Bucket>,
    public_base_url: String,
    expiry_secs: u32,
}

impl R2PhotoStorage {
    /// Construit le stockage depuis sa configuration.
    ///
    /// # Errors
    /// [`PhotoError::Backend`] si les identifiants ou le bucket sont invalides.
    pub fn new(config: R2Config) -> Result<Self, PhotoError> {
        let region = Region::Custom {
            region: config.region,
            endpoint: config.endpoint.trim_end_matches('/').to_owned(),
        };
        let credentials = Credentials::new(
            Some(&config.access_key),
            Some(&config.secret_key),
            None,
            None,
            None,
        )
        .map_err(|e| PhotoError::Backend(e.to_string()))?;
        let bucket = Bucket::new(&config.bucket, region, credentials)
            .map_err(|e| PhotoError::Backend(e.to_string()))?
            .with_path_style();
        Ok(Self {
            bucket,
            public_base_url: config.public_base_url.trim_end_matches('/').to_owned(),
            expiry_secs: config.expiry_secs,
        })
    }
}

#[async_trait::async_trait]
impl PhotoStorage for R2PhotoStorage {
    async fn presign_upload(&self, content_type: &str) -> Result<PhotoUpload, PhotoError> {
        let extension = photo_extension(content_type)
            .ok_or_else(|| PhotoError::UnsupportedType(content_type.to_owned()))?;
        // Clé opaque : évite les collisions et ne fuite pas le nom d'origine.
        let key = format!("recipes/{}.{extension}", uuid::Uuid::new_v4());
        let upload_url = self
            .bucket
            .presign_put(format!("/{key}"), self.expiry_secs, None, None)
            .await
            .map_err(|e| PhotoError::Backend(e.to_string()))?;
        Ok(PhotoUpload {
            upload_url,
            public_url: format!("{}/{key}", self.public_base_url),
        })
    }
}
