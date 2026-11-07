//! Couche domaine de `meal-plan` : le calendrier des repas.
//!
//! Modèle (cf. [plan.md — Modèle métier]) : pour un foyer, chaque **jour** ×
//! **créneau** (`lunch` / `dinner`) porte au plus un plat ([`Dish`]) : une
//! recette, ou un simple libellé libre (« Restes », « Resto »…) qui n'apporte
//! rien à la liste de courses. Une case est donc identifiée par
//! `(household_id, date, slot)` et pointe vers une [`RecipeId`] ou un
//! [`FreeLabel`]. Le domaine ne connaît la recette que par son identifiant
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

/// Longueur maximale d'un libellé libre, en caractères (alignée sur la
/// contrainte SQL).
pub const FREE_LABEL_MAX_CHARS: usize = 80;

/// Libellé d'un repas libre : texte non vide (espaces de bord retirés) d'au
/// plus [`FREE_LABEL_MAX_CHARS`] caractères.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FreeLabel(String);

impl FreeLabel {
    /// Valide un libellé saisi. `None` s'il est vide (après `trim`) ou trop long.
    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        let len = trimmed.chars().count();
        (1..=FREE_LABEL_MAX_CHARS)
            .contains(&len)
            .then(|| Self(trimmed.to_owned()))
    }

    /// Texte du libellé.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Ce qu'on mange sur un créneau.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dish {
    /// Une recette du foyer : ses ingrédients alimentent la liste de courses.
    Recipe(RecipeId),
    /// Un repas libre : simple libellé, sans ingrédients ni recette associée.
    Free(FreeLabel),
}

/// Une case du calendrier : un plat placé sur un créneau d'un jour, pour
/// un foyer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedMeal {
    /// Foyer propriétaire (scope).
    pub household_id: HouseholdId,
    /// Jour du repas.
    pub date: NaiveDate,
    /// Créneau (midi / soir).
    pub slot: Slot,
    /// Plat placé (recette ou repas libre).
    pub dish: Dish,
    /// Nombre de convives voulu pour ce créneau. La génération de liste de
    /// courses met les quantités de la recette à l'échelle par le ratio
    /// `servings / recette.servings`. Défaut : 2 (`kernel::DEFAULT_SERVINGS`).
    pub servings: u32,
}

impl PlannedMeal {
    /// Construit une case du calendrier. Une valeur de `servings` nulle est
    /// ramenée à 1 (invariant `> 0`, aligné sur la contrainte SQL) ; la
    /// validation de l'entrée utilisateur se fait en amont.
    #[must_use]
    pub fn new(
        household_id: HouseholdId,
        date: NaiveDate,
        slot: Slot,
        dish: Dish,
        servings: u32,
    ) -> Self {
        Self {
            household_id,
            date,
            slot,
            dish,
            servings: servings.max(1),
        }
    }
}

/// Port de persistance du calendrier. Déclaré ici (domaine), implémenté en
/// `infrastructure` (SQLx). Toutes les opérations sont scopées au foyer.
#[async_trait::async_trait]
pub trait MealPlanRepository: Send + Sync {
    /// Place (ou remplace) un plat sur un créneau. Upsert idempotent.
    ///
    /// # Errors
    /// [`RepositoryError::NotFound`] si le plat est une recette qui n'existe pas dans le foyer
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

    #[test]
    fn free_label_is_trimmed_and_bounded() {
        assert_eq!(FreeLabel::parse("  Restes ").unwrap().as_str(), "Restes");
        assert_eq!(FreeLabel::parse("   "), None);
        assert_eq!(FreeLabel::parse(""), None);
        let max = "é".repeat(FREE_LABEL_MAX_CHARS);
        assert!(FreeLabel::parse(&max).is_some());
        assert_eq!(FreeLabel::parse(&format!("{max}x")), None);
    }
}
