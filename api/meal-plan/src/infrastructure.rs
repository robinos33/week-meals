//! Couche infrastructure de `meal-plan` : implémentation SQLx du
//! [`MealPlanRepository`](crate::domain::MealPlanRepository).
//!
//! Requêtes runtime (aucune macro vérifiée à la compilation). Une violation de
//! la clé étrangère **vers les recettes** à l'écriture (recette absente du
//! foyer) est traduite en [`RepositoryError::NotFound`] ; les autres erreurs
//! SQLx en [`RepositoryError::Backend`].

use chrono::NaiveDate;
use kernel::{HouseholdId, RecipeId, RepositoryError};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::{MealPlanRepository, PlannedMeal, Slot};

/// Contrainte portant la FK vers les recettes (cf. migration `meal_plan`).
const RECIPE_FK: &str = "meal_plan_recipe_fkey";

/// Traduit une erreur SQLx. Seule la violation de la FK **vers les recettes**
/// devient `NotFound` : elle signifie que la recette n'appartient pas au foyer,
/// ce qui est une erreur d'appelant. La table porte une seconde FK (vers
/// `households`) dont la violation, elle, trahit une incohérence serveur — on
/// la laisse remonter en `Backend` plutôt que de la déguiser en « recette
/// introuvable ».
fn map_error(err: sqlx::Error) -> RepositoryError {
    if let sqlx::Error::Database(db) = &err {
        if db.is_foreign_key_violation() && db.constraint() == Some(RECIPE_FK) {
            return RepositoryError::NotFound;
        }
    }
    RepositoryError::Backend(err.to_string())
}

#[derive(sqlx::FromRow)]
struct MealRow {
    meal_date: NaiveDate,
    slot: String,
    recipe_id: Uuid,
}

impl MealRow {
    fn into_domain(self, household_id: HouseholdId) -> Result<PlannedMeal, RepositoryError> {
        let slot = Slot::parse(&self.slot).ok_or_else(|| {
            RepositoryError::Backend(format!("unknown stored slot: {}", self.slot))
        })?;
        Ok(PlannedMeal::new(
            household_id,
            self.meal_date,
            slot,
            RecipeId::from(self.recipe_id),
        ))
    }
}

/// Repository SQLx du calendrier.
#[derive(Clone)]
pub struct SqlxMealPlanRepository {
    pool: PgPool,
}

impl SqlxMealPlanRepository {
    /// Construit le repository à partir d'un pool Postgres.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl MealPlanRepository for SqlxMealPlanRepository {
    async fn set(&self, meal: &PlannedMeal) -> Result<(), RepositoryError> {
        // `counted_at` (garde du compteur « cuisiné X fois », #58) est remis à
        // zéro quand la recette du créneau **change** : la nouvelle recette
        // pourra être comptée à la prochaine génération. Reposer la même recette
        // n'y touche pas — sinon on la recompterait à tort.
        sqlx::query(
            "insert into meal_plan (household_id, meal_date, slot, recipe_id) \
             values ($1, $2, $3, $4) \
             on conflict (household_id, meal_date, slot) \
             do update set recipe_id = excluded.recipe_id, updated_at = now(), \
                 counted_at = case \
                     when meal_plan.recipe_id is distinct from excluded.recipe_id \
                     then null else meal_plan.counted_at end",
        )
        .bind(meal.household_id.as_uuid())
        .bind(meal.date)
        .bind(meal.slot.as_str())
        .bind(meal.recipe_id.as_uuid())
        .execute(&self.pool)
        .await
        .map_err(map_error)?;
        Ok(())
    }

    async fn clear(
        &self,
        household_id: HouseholdId,
        date: NaiveDate,
        slot: Slot,
    ) -> Result<(), RepositoryError> {
        let result = sqlx::query(
            "delete from meal_plan where household_id = $1 and meal_date = $2 and slot = $3",
        )
        .bind(household_id.as_uuid())
        .bind(date)
        .bind(slot.as_str())
        .execute(&self.pool)
        .await
        .map_err(map_error)?;
        if result.rows_affected() == 0 {
            Err(RepositoryError::NotFound)
        } else {
            Ok(())
        }
    }

    async fn week(
        &self,
        household_id: HouseholdId,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<PlannedMeal>, RepositoryError> {
        let rows: Vec<MealRow> = sqlx::query_as(
            "select meal_date, slot, recipe_id from meal_plan \
             where household_id = $1 and meal_date between $2 and $3 \
             order by meal_date, slot",
        )
        .bind(household_id.as_uuid())
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await
        .map_err(map_error)?;

        rows.into_iter()
            .map(|row| row.into_domain(household_id))
            .collect()
    }
}
