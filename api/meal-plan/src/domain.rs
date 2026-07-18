//! Couche domaine de `meal-plan` : le calendrier des repas.
//!
//! Modèle (cf. [plan.md — Modèle métier]) : pour un foyer, chaque **jour** ×
//! **créneau** (`lunch` / `dinner`) porte au plus une recette. Une case est
//! donc identifiée par `(household_id, date, slot)` et pointe vers une
//! [`RecipeId`]. Le domaine ne connaît la recette que par son identifiant
//! (transverse, dans le `kernel`) : aucun couplage avec le domaine `recipes`.
//!
//! [plan.md — Modèle métier]: ../../../docs/plan.md

use chrono::NaiveDate;
use kernel::{HouseholdId, RecipeId, RepositoryError};

/// Créneau d'un repas dans une journée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Slot {
    /// Midi.
    Lunch,
    /// Soir.
    Dinner,
}

impl Slot {
    /// Représentation textuelle canonique (contrat partagé avec l'enum SQL).
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Slot::Lunch => "lunch",
            Slot::Dinner => "dinner",
        }
    }

    /// Interprète un créneau depuis sa forme textuelle.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "lunch" => Some(Slot::Lunch),
            "dinner" => Some(Slot::Dinner),
            _ => None,
        }
    }
}

impl std::fmt::Display for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Une case du calendrier : une recette placée sur un créneau d'un jour, pour
/// un foyer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedMeal {
    /// Foyer propriétaire (scope).
    pub household_id: HouseholdId,
    /// Jour du repas.
    pub date: NaiveDate,
    /// Créneau (midi / soir).
    pub slot: Slot,
    /// Recette placée.
    pub recipe_id: RecipeId,
}

impl PlannedMeal {
    /// Construit une case du calendrier.
    #[must_use]
    pub fn new(
        household_id: HouseholdId,
        date: NaiveDate,
        slot: Slot,
        recipe_id: RecipeId,
    ) -> Self {
        Self {
            household_id,
            date,
            slot,
            recipe_id,
        }
    }
}

/// Port de persistance du calendrier. Déclaré ici (domaine), implémenté en
/// `infrastructure` (SQLx). Toutes les opérations sont scopées au foyer.
#[async_trait::async_trait]
pub trait MealPlanRepository: Send + Sync {
    /// Place (ou remplace) une recette sur un créneau. Upsert idempotent.
    ///
    /// # Errors
    /// [`RepositoryError::NotFound`] si la recette n'existe pas dans le foyer
    /// (violation d'intégrité référentielle) ; [`RepositoryError::Backend`]
    /// sur panne technique.
    async fn set(&self, meal: &PlannedMeal) -> Result<(), RepositoryError>;

    /// Vide un créneau.
    ///
    /// # Errors
    /// [`RepositoryError::NotFound`] si le créneau était déjà vide.
    async fn clear(
        &self,
        household_id: HouseholdId,
        date: NaiveDate,
        slot: Slot,
    ) -> Result<(), RepositoryError>;

    /// Lit les cases occupées d'un foyer sur une plage de jours (bornes
    /// incluses), ordonnées par date puis créneau.
    async fn week(
        &self,
        household_id: HouseholdId,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<Vec<PlannedMeal>, RepositoryError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_round_trips_through_text() {
        assert_eq!(Slot::parse("lunch"), Some(Slot::Lunch));
        assert_eq!(Slot::parse("dinner"), Some(Slot::Dinner));
        assert_eq!(Slot::parse("brunch"), None);
        assert_eq!(Slot::Lunch.as_str(), "lunch");
        assert_eq!(Slot::Dinner.to_string(), "dinner");
    }
}
