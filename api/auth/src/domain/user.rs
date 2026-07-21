//! Un utilisateur (`User`) : membre d'un foyer, identifié par un pseudo. Pas
//! d'email ni de mot de passe (cf. ADR-0006) — l'identité est portée par les
//! passkeys de ses appareils. Le pseudo n'est qu'un **libellé d'affichage** :
//! il n'est plus unique globalement (il ne sert plus à se connecter).

use kernel::{HouseholdId, UserId};

use super::AuthError;

/// Longueur maximale d'un pseudo.
const USERNAME_MAX_LEN: usize = 32;

/// Pseudo d'un utilisateur : chaîne non vide, bornée en longueur.
///
/// Trim appliqué à la construction. Simple libellé d'affichage depuis
/// l'ADR-0006 (plus de contrainte d'unicité).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Username(String);

impl Username {
    /// Construit un pseudo valide.
    ///
    /// # Errors
    /// - [`AuthError::EmptyUsername`] si vide (après trim).
    /// - [`AuthError::UsernameTooLong`] au-delà de [`USERNAME_MAX_LEN`].
    pub fn new(value: impl Into<String>) -> Result<Self, AuthError> {
        let value = value.into().trim().to_owned();
        if value.is_empty() {
            return Err(AuthError::EmptyUsername);
        }
        if value.chars().count() > USERNAME_MAX_LEN {
            return Err(AuthError::UsernameTooLong {
                max: USERNAME_MAX_LEN,
            });
        }
        Ok(Self(value))
    }

    /// Le pseudo sous forme de `&str`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Username {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Un utilisateur : membre d'un foyer, avec un pseudo d'affichage. Son identité
/// est portée par les passkeys de ses [`Device`](super::device::Device)s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    /// Identifiant de l'utilisateur (aussi le handle WebAuthn, cf. ADR-0006).
    pub id: UserId,
    /// Foyer d'appartenance (toutes les données y sont scopées).
    pub household_id: HouseholdId,
    /// Pseudo d'affichage.
    pub username: Username,
}

impl User {
    /// Crée un utilisateur avec un identifiant frais.
    #[must_use]
    pub fn new(household_id: HouseholdId, username: Username) -> Self {
        Self {
            id: UserId::new(),
            household_id,
            username,
        }
    }

    /// Reconstitue un utilisateur depuis la persistance (identifiant connu).
    #[must_use]
    pub fn from_parts(id: UserId, household_id: HouseholdId, username: Username) -> Self {
        Self {
            id,
            household_id,
            username,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_and_rejects_blank_usernames() {
        assert_eq!(Username::new("  robin ").unwrap().as_str(), "robin");
        assert_eq!(Username::new("  ").unwrap_err(), AuthError::EmptyUsername);
    }

    #[test]
    fn rejects_overlong_usernames() {
        let long = "a".repeat(USERNAME_MAX_LEN + 1);
        assert_eq!(
            Username::new(long).unwrap_err(),
            AuthError::UsernameTooLong {
                max: USERNAME_MAX_LEN
            }
        );
    }
}
