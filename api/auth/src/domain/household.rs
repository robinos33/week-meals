//! Le foyer (`Household`) : racine de scoping. Toutes les données applicatives
//! (recettes, calendrier, courses) appartiennent à un foyer — l'app est
//! multi-foyers *by design* (cf. `plan.md`).

use kernel::HouseholdId;

use super::AuthError;

/// Nom d'un foyer : chaîne non vide (trim appliqué à la construction).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HouseholdName(String);

impl HouseholdName {
    /// Construit un nom de foyer valide.
    ///
    /// # Errors
    /// [`AuthError::EmptyHouseholdName`] si la chaîne est vide (après trim).
    pub fn new(value: impl Into<String>) -> Result<Self, AuthError> {
        let value = value.into().trim().to_owned();
        if value.is_empty() {
            return Err(AuthError::EmptyHouseholdName);
        }
        Ok(Self(value))
    }

    /// Le nom sous forme de `&str`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for HouseholdName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Premier jour de la semaine d'un foyer (#57), paramétrable car le planning
/// est partagé et notre semaine réelle démarre le samedi (courses).
///
/// Convention `Date.getDay()` du front : `0` = dimanche … `6` = samedi. Le
/// défaut applicatif est lundi (`1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeekStartDay(u8);

impl WeekStartDay {
    /// Lundi — défaut historique, avant que le réglage n'existe.
    pub const MONDAY: Self = Self(1);

    /// Construit un jour de départ valide (`0..=6`).
    ///
    /// # Errors
    /// [`AuthError::InvalidWeekStartDay`] hors de `0..=6`.
    pub fn new(value: u8) -> Result<Self, AuthError> {
        if value > 6 {
            return Err(AuthError::InvalidWeekStartDay);
        }
        Ok(Self(value))
    }

    /// La valeur brute (`0` = dimanche … `6` = samedi).
    #[must_use]
    pub fn value(self) -> u8 {
        self.0
    }
}

/// Un foyer : entité racine de scoping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Household {
    /// Identifiant du foyer.
    pub id: HouseholdId,
    /// Nom du foyer.
    pub name: HouseholdName,
}

impl Household {
    /// Crée un foyer avec un identifiant frais.
    #[must_use]
    pub fn new(name: HouseholdName) -> Self {
        Self {
            id: HouseholdId::new(),
            name,
        }
    }

    /// Reconstitue un foyer depuis la persistance (identifiant connu).
    #[must_use]
    pub fn from_parts(id: HouseholdId, name: HouseholdName) -> Self {
        Self { id, name }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_and_rejects_blank_names() {
        assert_eq!(
            HouseholdName::new("  Chez nous ").unwrap().as_str(),
            "Chez nous"
        );
        assert_eq!(
            HouseholdName::new("   ").unwrap_err(),
            AuthError::EmptyHouseholdName
        );
    }

    #[test]
    fn new_household_gets_a_fresh_id() {
        let a = Household::new(HouseholdName::new("A").unwrap());
        let b = Household::new(HouseholdName::new("B").unwrap());
        assert_ne!(a.id, b.id);
    }
}
