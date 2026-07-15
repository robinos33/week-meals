//! Un utilisateur (`User`) : membre d'un foyer, identifié par un pseudo. Pas
//! d'email (cf. ADR-0002). Le pseudo est unique globalement — il suffit à
//! identifier l'utilisateur au login, sans sélection de foyer.

use kernel::{HouseholdId, UserId};

use super::password::PasswordHash;
use super::AuthError;

/// Longueur maximale d'un pseudo.
const USERNAME_MAX_LEN: usize = 32;

/// Pseudo d'un utilisateur : chaîne non vide, bornée en longueur.
///
/// Trim appliqué à la construction. La comparaison reste sensible à la casse :
/// l'unicité globale est portée par la contrainte SQL `users.username unique`.
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

/// Un utilisateur : membre d'un foyer, avec pseudo et hash de mot de passe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    /// Identifiant de l'utilisateur.
    pub id: UserId,
    /// Foyer d'appartenance (toutes les données y sont scopées).
    pub household_id: HouseholdId,
    /// Pseudo (unique globalement).
    pub username: Username,
    /// Hash Argon2id du mot de passe.
    pub password_hash: PasswordHash,
}

impl User {
    /// Crée un utilisateur avec un identifiant frais.
    #[must_use]
    pub fn new(household_id: HouseholdId, username: Username, password_hash: PasswordHash) -> Self {
        Self {
            id: UserId::new(),
            household_id,
            username,
            password_hash,
        }
    }

    /// Reconstitue un utilisateur depuis la persistance (identifiant connu).
    #[must_use]
    pub fn from_parts(
        id: UserId,
        household_id: HouseholdId,
        username: Username,
        password_hash: PasswordHash,
    ) -> Self {
        Self {
            id,
            household_id,
            username,
            password_hash,
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
