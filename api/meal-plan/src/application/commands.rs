//! Use cases d'écriture du calendrier : placer un plat (recette ou repas libre)
//! sur un créneau,
//! vider un créneau. Scopés au foyer.

use chrono::NaiveDate;
use kernel::{HouseholdId, RepositoryError};

use crate::domain::{Dish, MealPlanRepository, PlannedMeal, Slot};

// --- Place ----------------------------------------------------------------

/// Command : placer (ou remplacer) un plat sur un créneau.
#[derive(Debug, Clone)]
pub struct PlaceMealCommand {
    /// Foyer propriétaire (scope).
    pub household_id: HouseholdId,
    /// Jour.
    pub date: NaiveDate,
    /// Créneau.
    pub slot: Slot,
    /// Plat à placer : recette ou repas libre.
    pub dish: Dish,
    /// Nombre de convives pour ce créneau (base de la mise à l'échelle des
    /// courses). Défaut applicatif : `kernel::DEFAULT_SERVINGS`.
    pub servings: u32,
}

/// Résultat d'un placement.
#[derive(Debug, PartialEq, Eq)]
pub enum PlaceMealResponse {
    /// Plat placé sur le créneau.
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
            command.dish,
            command.servings,
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
    use crate::domain::FreeLabel;
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
                dish: Dish::Recipe(recipe),
                servings: 2,
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
                dish: Dish::Recipe(RecipeId::new()),
                servings: 2,
            })
            .await;
        let second = RecipeId::new();
        handler
            .handle(PlaceMealCommand {
                household_id: household,
                date: date("2026-07-13"),
                slot: Slot::Lunch,
                dish: Dish::Recipe(second),
                servings: 2,
            })
            .await;

        assert_eq!(plan.count(), 1);
    }

    #[tokio::test]
    async fn free_meal_replaces_a_recipe() {
        let plan = InMemoryMealPlan::default();
        let household = HouseholdId::new();
        let handler = PlaceMealHandler::new(&plan);
        let slot = |dish| PlaceMealCommand {
            household_id: household,
            date: date("2026-07-13"),
            slot: Slot::Dinner,
            dish,
            servings: 2,
        };

        handler.handle(slot(Dish::Recipe(RecipeId::new()))).await;
        let free = Dish::Free(FreeLabel::parse("Restes").unwrap());
        assert_eq!(
            handler.handle(slot(free.clone())).await,
            PlaceMealResponse::Placed
        );

        assert_eq!(plan.count(), 1);
        assert_eq!(plan.only().dish, free);
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
