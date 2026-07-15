//! Use case de lecture du calendrier : lire la semaine (une plage de jours).

use chrono::NaiveDate;
use kernel::HouseholdId;

use crate::domain::{MealPlanRepository, PlannedMeal};

/// Query : lire les cases occupées d'un foyer sur une plage de jours.
#[derive(Debug, Clone)]
pub struct GetWeekQuery {
    /// Foyer propriétaire (scope).
    pub household_id: HouseholdId,
    /// Premier jour (inclus).
    pub start: NaiveDate,
    /// Dernier jour (inclus).
    pub end: NaiveDate,
}

/// Résultat d'une lecture de semaine.
#[derive(Debug)]
pub enum GetWeekResponse {
    /// Cases occupées (ordonnées par date puis créneau). Vide si la plage est
    /// inversée ou sans repas planifié.
    Week(Vec<PlannedMeal>),
    /// Panne technique.
    Unavailable,
}

/// Handler de la lecture de semaine.
pub struct GetWeekHandler<'a> {
    plan: &'a dyn MealPlanRepository,
}

impl<'a> GetWeekHandler<'a> {
    /// Construit le handler.
    #[must_use]
    pub fn new(plan: &'a dyn MealPlanRepository) -> Self {
        Self { plan }
    }

    /// Exécute la lecture. Une plage inversée renvoie une semaine vide (aucune
    /// requête). Ne renvoie jamais d'erreur.
    pub async fn handle(&self, query: GetWeekQuery) -> GetWeekResponse {
        if query.end < query.start {
            return GetWeekResponse::Week(Vec::new());
        }
        match self
            .plan
            .week(query.household_id, query.start, query.end)
            .await
        {
            Ok(meals) => GetWeekResponse::Week(meals),
            Err(_) => GetWeekResponse::Unavailable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Slot;
    use crate::testing::{date, InMemoryMealPlan};
    use kernel::RecipeId;

    #[tokio::test]
    async fn week_returns_slots_in_range() {
        let household = HouseholdId::new();
        let plan = InMemoryMealPlan::default();
        plan.seed(PlannedMeal::new(
            household,
            date("2026-07-13"),
            Slot::Lunch,
            RecipeId::new(),
        ));
        plan.seed(PlannedMeal::new(
            household,
            date("2026-07-20"),
            Slot::Dinner,
            RecipeId::new(),
        ));

        let response = GetWeekHandler::new(&plan)
            .handle(GetWeekQuery {
                household_id: household,
                start: date("2026-07-13"),
                end: date("2026-07-19"),
            })
            .await;
        match response {
            GetWeekResponse::Week(meals) => assert_eq!(meals.len(), 1),
            other => panic!("attendu Week, obtenu {other:?}"),
        }
    }

    #[tokio::test]
    async fn inverted_range_yields_empty_week() {
        let plan = InMemoryMealPlan::default();
        let response = GetWeekHandler::new(&plan)
            .handle(GetWeekQuery {
                household_id: HouseholdId::new(),
                start: date("2026-07-20"),
                end: date("2026-07-13"),
            })
            .await;
        assert!(matches!(response, GetWeekResponse::Week(meals) if meals.is_empty()));
    }
}
