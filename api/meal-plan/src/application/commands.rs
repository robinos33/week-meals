//! Use cases d'écriture du calendrier : placer une recette sur un créneau,
//! vider un créneau. Scopés au foyer.

use chrono::NaiveDate;
use kernel::{HouseholdId, RecipeId, RepositoryError};

use crate::domain::{MealPlanRepository, PlannedMeal, Slot};

// --- Place ----------------------------------------------------------------

/// Command : placer (ou remplacer) une recette sur un créneau.
#[derive(Debug, Clone)]
pub struct PlaceMealCommand {
    /// Foyer propriétaire (scope).
    pub household_id: HouseholdId,
    /// Jour.
    pub date: NaiveDate,
    /// Créneau.
    pub slot: Slot,
    /// Recette à placer.
    pub recipe_id: RecipeId,
}

/// Résultat d'un placement.
#[derive(Debug, PartialEq, Eq)]
pub enum PlaceMealResponse {
    /// Recette placée sur le créneau.
    Placed,
    /// La recette n'existe pas dans le foyer.
    RecipeNotFound,
    /// Panne technique.
    Unavailable,
}

/// Handler du placement.
pub struct PlaceMealHandler<'a> {
    plan: &'a dyn MealPlanRepository,
}

impl<'a> PlaceMealHandler<'a> {
    /// Construit le handler.
    #[must_use]
    pub fn new(plan: &'a dyn MealPlanRepository) -> Self {
        Self { plan }
    }

    /// Exécute le placement. Ne renvoie jamais d'erreur.
    pub async fn handle(&self, command: PlaceMealCommand) -> PlaceMealResponse {
        let meal = PlannedMeal::new(
            command.household_id,
            command.date,
            command.slot,
            command.recipe_id,
        );
        match self.plan.set(&meal).await {
            Ok(()) => PlaceMealResponse::Placed,
            Err(RepositoryError::NotFound) => PlaceMealResponse::RecipeNotFound,
            Err(_) => PlaceMealResponse::Unavailable,
        }
    }
}

// --- Clear ----------------------------------------------------------------

/// Command : vider un créneau.
#[derive(Debug, Clone)]
pub struct ClearMealCommand {
    /// Foyer propriétaire (scope).
    pub household_id: HouseholdId,
    /// Jour.
    pub date: NaiveDate,
    /// Créneau.
    pub slot: Slot,
}

/// Résultat d'un vidage.
#[derive(Debug, PartialEq, Eq)]
pub enum ClearMealResponse {
    /// Créneau vidé.
    Cleared,
    /// Le créneau était déjà vide.
    NotFound,
    /// Panne technique.
    Unavailable,
}

/// Handler du vidage.
pub struct ClearMealHandler<'a> {
    plan: &'a dyn MealPlanRepository,
}

impl<'a> ClearMealHandler<'a> {
    /// Construit le handler.
    #[must_use]
    pub fn new(plan: &'a dyn MealPlanRepository) -> Self {
        Self { plan }
    }

    /// Exécute le vidage. Ne renvoie jamais d'erreur.
    pub async fn handle(&self, command: ClearMealCommand) -> ClearMealResponse {
        match self
            .plan
            .clear(command.household_id, command.date, command.slot)
            .await
        {
            Ok(()) => ClearMealResponse::Cleared,
            Err(RepositoryError::NotFound) => ClearMealResponse::NotFound,
            Err(_) => ClearMealResponse::Unavailable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{date, InMemoryMealPlan};
    use kernel::RecipeId;

    #[tokio::test]
    async fn place_then_clear() {
        let plan = InMemoryMealPlan::default();
        let household = HouseholdId::new();
        let recipe = RecipeId::new();

        let placed = PlaceMealHandler::new(&plan)
            .handle(PlaceMealCommand {
                household_id: household,
                date: date("2026-07-13"),
                slot: Slot::Dinner,
                recipe_id: recipe,
            })
            .await;
        assert_eq!(placed, PlaceMealResponse::Placed);
        assert_eq!(plan.count(), 1);

        let cleared = ClearMealHandler::new(&plan)
            .handle(ClearMealCommand {
                household_id: household,
                date: date("2026-07-13"),
                slot: Slot::Dinner,
            })
            .await;
        assert_eq!(cleared, ClearMealResponse::Cleared);
        assert_eq!(plan.count(), 0);
    }

    #[tokio::test]
    async fn placing_twice_replaces_the_slot() {
        let plan = InMemoryMealPlan::default();
        let household = HouseholdId::new();
        let handler = PlaceMealHandler::new(&plan);

        handler
            .handle(PlaceMealCommand {
                household_id: household,
                date: date("2026-07-13"),
                slot: Slot::Lunch,
                recipe_id: RecipeId::new(),
            })
            .await;
        let second = RecipeId::new();
        handler
            .handle(PlaceMealCommand {
                household_id: household,
                date: date("2026-07-13"),
                slot: Slot::Lunch,
                recipe_id: second,
            })
            .await;

        assert_eq!(plan.count(), 1);
    }

    #[tokio::test]
    async fn clearing_empty_slot_is_not_found() {
        let plan = InMemoryMealPlan::default();
        let response = ClearMealHandler::new(&plan)
            .handle(ClearMealCommand {
                household_id: HouseholdId::new(),
                date: date("2026-07-13"),
                slot: Slot::Lunch,
            })
            .await;
        assert_eq!(response, ClearMealResponse::NotFound);
    }
}
