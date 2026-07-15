//! Le foyer ([`Household`]) et son nom validé ([`HouseholdName`]).

use std::fmt;

use kernel::HouseholdId;
use thiserror::Error;

/// Longueur maximale du nom d'un foyer (en caractères Unicode).
pub const MAX_HOUSEHOLD_NAME_LEN: usize = 60;

/// Nom d'un foyer : non vide (espaces rognés), borné en longueur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HouseholdName(String);

/// Erreurs de validation d'un [`HouseholdName`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum HouseholdNameError {
    /// Nom vide (ou uniquement des espaces).
    #[error("nom de foyer vide")]
    Empty,
    /// Nom dépassant [`MAX_HOUSEHOLD_NAME_LEN`] caractères.
    #[error("nom de foyer trop long (maximum {max} caractères)")]
    TooLong {
        /// Longueur maximale autorisée.
        max: usize,
    },
}

impl HouseholdName {
    /// Valide et construit un nom de foyer. Les espaces de début/fin sont
    /// rognés.
    ///
    /// # Errors
    ///
    /// Renvoie [`HouseholdNameError`] si le nom est vide ou trop long.
    pub fn new(raw: impl AsRef<str>) -> Result<Self, HouseholdNameError> {
        let trimmed = raw.as_ref().trim();
        if trimmed.is_empty() {
            return Err(HouseholdNameError::Empty);
        }
        if trimmed.chars().count() > MAX_HOUSEHOLD_NAME_LEN {
            return Err(HouseholdNameError::TooLong {
                max: MAX_HOUSEHOLD_NAME_LEN,
            });
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Le nom sous forme de chaîne.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HouseholdName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Un foyer : unité de scoping de toutes les données de l'app.
#[derive(Debug, Clone)]
pub struct Household {
    id: HouseholdId,
    name: HouseholdName,
}

impl Household {
    /// Crée un nouveau foyer avec un identifiant fraîchement généré.
    #[must_use]
    pub fn new(name: HouseholdName) -> Self {
        Self {
            id: HouseholdId::new(),
            name,
        }
    }

    /// Reconstruit un foyer depuis la persistance (identifiant connu).
    #[must_use]
    pub fn from_parts(id: HouseholdId, name: HouseholdName) -> Self {
        Self { id, name }
    }

    /// Identifiant du foyer.
    #[must_use]
    pub fn id(&self) -> HouseholdId {
        self.id
    }

    /// Nom du foyer.
    #[must_use]
    pub fn name(&self) -> &HouseholdName {
        &self.name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rogne_les_espaces() {
        let name = HouseholdName::new("  Chez nous  ").unwrap();
        assert_eq!(name.as_str(), "Chez nous");
    }

    #[test]
    fn rejette_un_nom_vide() {
        assert_eq!(HouseholdName::new("   "), Err(HouseholdNameError::Empty));
    }

    #[test]
    fn rejette_un_nom_trop_long() {
        let raw = "a".repeat(MAX_HOUSEHOLD_NAME_LEN + 1);
        assert!(matches!(
            HouseholdName::new(raw),
            Err(HouseholdNameError::TooLong { .. })
        ));
    }

    #[test]
    fn new_genere_un_identifiant() {
        let a = Household::new(HouseholdName::new("A").unwrap());
        let b = Household::new(HouseholdName::new("A").unwrap());
        assert_ne!(a.id(), b.id());
    }
}
