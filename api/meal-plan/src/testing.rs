//! Utilitaires de test : un [`MealPlanRepository`] en mémoire. Compilé
//! uniquement pour les tests.

use std::sync::Mutex;

use chrono::NaiveDate;
use kernel::{HouseholdId, RepositoryError};

use crate::domain::{MealPlanRepository, PlannedMeal, Slot};

/// Raccourci de construction d'une date ISO dans les tests.
#[must_use]
pub fn date(iso: &str) -> NaiveDate {
    iso.parse().expect("date ISO valide")
}

/// Calendrier en mémoire, scopé au foyer comme l'implémentation SQLx.
#[derive(Default)]
pub struct InMemoryMealPlan {
    meals: Mutex<Vec<PlannedMeal>>,
}

impl InMemoryMealPlan {
    /// Insère directement une case (préparation de test).
    pub fn seed(&self, meal: PlannedMeal) {
        self.meals.lock().unwrap().push(meal);
    }

    /// Nombre de cases occupées.
    #[must_use]
    pub fn count(&self) -> usize {
        self.meals.lock().unwrap().len()
    }
}

#[async_trait::async_trait]
impl MealPlanRepository for InMemoryMealPlan {
    async fn set(&self, meal: &PlannedMeal) -> Result<(), RepositoryError> {
        let mut meals = self.meals.lock().unwrap();
        meals.retain(|m| {
            !(m.household_id == meal.household_id && m.date == meal.date && m.slot == meal.slot)
        });
        meals.push(meal.clone());
        Ok(())
    }

    async fn clear(
        &self,
        household_id: HouseholdId,
        date: NaiveDate,
        slot: Slot,
    ) -> Result<(), RepositoryError> {
        let mut meals = self.meals.lock().unwrap();
        let before = meals.len();
        meals.retain(|m| !(m.household_id == household_id && m.date == date && m.slot == slot));
        if meals.len() == before {
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
        let mut found: Vec<PlannedMeal> = self
            .meals
            .lock()
            .unwrap()
            .iter()
            .filter(|m| m.household_id == household_id && m.date >= start && m.date <= end)
            .cloned()
            .collect();
        found.sort_by(|a, b| (a.date, a.slot.as_str()).cmp(&(b.date, b.slot.as_str())));
        Ok(found)
    }
}
