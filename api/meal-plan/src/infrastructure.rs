//! Couche infrastructure de `meal-plan` : implémentation SQLx du
//! [`MealPlanRepository`](crate::domain::MealPlanRepository).
//!
//! Requêtes runtime (aucune macro vérifiée à la compilation). Une case porte
//! soit `recipe_id`, soit `label` (repas libre). Planifier une
//! recette absente du foyer donne [`RepositoryError::NotFound`] ; les autres
//! erreurs SQLx deviennent [`RepositoryError::Backend`].

use chrono::NaiveDate;
use kernel::{HouseholdId, RecipeId, RepositoryError, DEFAULT_SERVINGS};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::domain::{Dish, FreeLabel, MealPlanRepository, PlannedMeal, Slot};

/// Traduit une erreur SQLx en erreur de repository agnostique.
///
/// Postgres nommait la contrainte violée, ce qui permettait de distinguer après
/// coup « recette hors foyer » (erreur d'appelant, 404) de la violation de la FK
/// vers `households` (incohérence serveur, 500). SQLite ne donne pas ce nom :
/// l'appartenance de la recette est donc vérifiée *avant* l'écriture (cf.
/// ADR-0008), et toute erreur qui remonte jusqu'ici est bien une panne.
fn backend(err: sqlx::Error) -> RepositoryError {
    RepositoryError::Backend(err.to_string())
}

#[derive(sqlx::FromRow)]
struct MealRow {
    meal_date: NaiveDate,
    slot: String,
    recipe_id: Option<Uuid>,
    label: Option<String>,
    servings: i32,
}

impl MealRow {
    fn into_domain(self, household_id: HouseholdId) -> Result<PlannedMeal, RepositoryError> {
        let slot = Slot::parse(&self.slot).ok_or_else(|| {
            RepositoryError::Backend(format!("unknown stored slot: {}", self.slot))
        })?;
        // `servings` est contraint `> 0` en base ; un zéro/négatif serait une
        // corruption — on retombe sur la base par défaut.
        let servings = u32::try_from(self.servings)
            .ok()
            .filter(|&s| s > 0)
            .unwrap_or(DEFAULT_SERVINGS);
        // Le check XOR de la table garantit exactement l'un des deux.
        let dish = match (self.recipe_id, self.label) {
            (Some(recipe_id), None) => Dish::Recipe(RecipeId::from(recipe_id)),
            (None, Some(label)) => Dish::Free(FreeLabel::parse(&label).ok_or_else(|| {
                RepositoryError::Backend(format!("invalid stored label: {label:?}"))
            })?),
            _ => {
                return Err(RepositoryError::Backend(
                    "stored meal must have exactly one of recipe_id / label".to_owned(),
                ))
            }
        };
        Ok(PlannedMeal::new(
            household_id,
            self.meal_date,
            slot,
            dish,
            servings,
        ))
    }
}

/// Repository SQLx du calendrier.
#[derive(Clone)]
pub struct SqlxMealPlanRepository {
    pool: SqlitePool,
}

impl SqlxMealPlanRepository {
    /// Construit le repository à partir d'un pool SQLite.
    #[must_use]
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Vérifie que la recette appartient bien au foyer.
    ///
    /// Tient lieu du diagnostic que Postgres livrait dans le nom de la
    /// contrainte violée. La FK composite reste en base : entre ce contrôle et
    /// l'`insert`, elle garde l'intégrité si la recette disparaît — l'appel
    /// échouerait alors en `Backend`, ce qui est la bonne lecture (personne
    /// n'a demandé une recette inexistante, elle s'est volatilisée).
    async fn recipe_belongs(
        &self,
        household_id: HouseholdId,
        recipe_id: RecipeId,
    ) -> Result<bool, RepositoryError> {
        let found: Option<(i64,)> =
            sqlx::query_as("select 1 from recipes where id = ? and household_id = ?")
                .bind(recipe_id.as_uuid())
                .bind(household_id.as_uuid())
                .fetch_optional(&self.pool)
                .await
                .map_err(backend)?;
        Ok(found.is_some())
    }
}

#[async_trait::async_trait]
impl MealPlanRepository for SqlxMealPlanRepository {
    async fn set(&self, meal: &PlannedMeal) -> Result<(), RepositoryError> {
        // `counted_at` (garde du compteur « cuisiné X fois », #58) est remis à
        // zéro quand la recette du créneau **change** : la nouvelle recette
        // pourra être comptée à la prochaine génération. Reposer la même recette
        // n'y touche pas — sinon on la recompterait à tort. Un repas libre n'a
        // pas de recette (`recipe_id` NULL) : `is not` compare bien les NULL.
        let (recipe_id, label) = match &meal.dish {
            Dish::Recipe(recipe_id) => {
                if !self.recipe_belongs(meal.household_id, *recipe_id).await? {
                    return Err(RepositoryError::NotFound);
                }
                (Some(recipe_id.as_uuid()), None)
            }
            Dish::Free(label) => (None, Some(label.as_str())),
        };
        sqlx::query(
            "insert into meal_plan (household_id, meal_date, slot, recipe_id, label, servings) \
             values (?, ?, ?, ?, ?, ?) \
             on conflict (household_id, meal_date, slot) \
             do update set recipe_id = excluded.recipe_id, label = excluded.label, \
                 servings = excluded.servings, updated_at = datetime('now'), \
                 counted_at = case \
                     when meal_plan.recipe_id is not excluded.recipe_id \
                     then null else meal_plan.counted_at end",
        )
        .bind(meal.household_id.as_uuid())
        .bind(meal.date)
        .bind(meal.slot.as_str())
        .bind(recipe_id)
        .bind(label)
        .bind(i32::try_from(meal.servings).unwrap_or(i32::MAX))
        .execute(&self.pool)
        .await
        .map_err(backend)?;
        Ok(())
    }

    async fn clear(
        &self,
        household_id: HouseholdId,
        date: NaiveDate,
        slot: Slot,
    ) -> Result<(), RepositoryError> {
        let result = sqlx::query(
            "delete from meal_plan where household_id = ? and meal_date = ? and slot = ?",
        )
        .bind(household_id.as_uuid())
        .bind(date)
        .bind(slot.as_str())
        .execute(&self.pool)
        .await
        .map_err(backend)?;
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
            "select meal_date, slot, recipe_id, label, servings from meal_plan \
             where household_id = ? and meal_date between ? and ? \
             order by meal_date, slot",
        )
        .bind(household_id.as_uuid())
        .bind(start)
        .bind(end)
        .fetch_all(&self.pool)
        .await
        .map_err(backend)?;

        rows.into_iter()
            .map(|row| row.into_domain(household_id))
            .collect()
    }
}
