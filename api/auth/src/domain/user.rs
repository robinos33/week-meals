//! L'utilisateur ([`User`]) et son pseudo validé ([`Username`]).

use std::fmt;

use kernel::{HouseholdId, UserId};
use thiserror::Error;

use super::password::PasswordHash;

/// Longueur minimale d'un pseudo (en caractères Unicode).
pub const MIN_USERNAME_LEN: usize = 2;
/// Longueur maximale d'un pseudo (en caractères Unicode).
pub const MAX_USERNAME_LEN: usize = 32;

/// Pseudo d'un utilisateur : pas d'email (cf. ADR-0002).
///
/// Rogné, borné en longueur, restreint aux lettres/chiffres et à `_`/`-`.
/// L'unicité n'est garantie qu'**au sein d'un foyer** (voir
/// [`UserRepository::find_by_username`](super::repository::UserRepository::find_by_username)).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Username(String);

/// Erreurs de validation d'un [`Username`].
#[derive(Debug, Error, PartialEq, Eq)]
pub enum UsernameError {
    /// Pseudo vide (ou uniquement des espaces).
    #[error("pseudo vide")]
    Empty,
    /// Pseudo plus court que [`MIN_USERNAME_LEN`].
    #[error("pseudo trop court (minimum {min} caractères)")]
    TooShort {
        /// Longueur minimale requise.
        min: usize,
    },
    /// Pseudo plus long que [`MAX_USERNAME_LEN`].
    #[error("pseudo trop long (maximum {max} caractères)")]
    TooLong {
        /// Longueur maximale autorisée.
        max: usize,
    },
    /// Présence d'un caractère hors de l'alphabet autorisé.
    #[error("caractère invalide dans le pseudo (autorisés : lettres, chiffres, '_' et '-')")]
    InvalidChar,
}

impl Username {
    /// Valide et construit un pseudo. Les espaces de début/fin sont rognés.
    ///
    /// # Errors
    ///
    /// Renvoie [`UsernameError`] si le pseudo est vide, hors bornes de longueur
    /// ou contient un caractère non autorisé.
    pub fn new(raw: impl AsRef<str>) -> Result<Self, UsernameError> {
        let trimmed = raw.as_ref().trim();
        if trimmed.is_empty() {
            return Err(UsernameError::Empty);
        }
        let len = trimmed.chars().count();
        if len < MIN_USERNAME_LEN {
            return Err(UsernameError::TooShort {
                min: MIN_USERNAME_LEN,
            });
        }
        if len > MAX_USERNAME_LEN {
            return Err(UsernameError::TooLong {
                max: MAX_USERNAME_LEN,
            });
        }
        if !trimmed
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(UsernameError::InvalidChar);
        }
        Ok(Self(trimmed.to_owned()))
    }

    /// Le pseudo sous forme de chaîne.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Username {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Un utilisateur, rattaché à un foyer. Aucune donnée personnelle : pseudo +
/// hash du mot de passe (cf. ADR-0002).
#[derive(Debug, Clone)]
pub struct User {
    id: UserId,
    household_id: HouseholdId,
    username: Username,
    password_hash: PasswordHash,
}

impl User {
    /// Crée un nouvel utilisateur (identifiant fraîchement généré) rattaché à
    /// un foyer.
    #[must_use]
    pub fn new(household_id: HouseholdId, username: Username, password_hash: PasswordHash) -> Self {
        Self {
            id: UserId::new(),
            household_id,
            username,
            password_hash,
        }
    }

    /// Reconstruit un utilisateur depuis la persistance (identifiant connu).
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

    /// Identifiant de l'utilisateur.
    #[must_use]
    pub fn id(&self) -> UserId {
        self.id
    }

    /// Foyer auquel l'utilisateur est rattaché.
    #[must_use]
    pub fn household_id(&self) -> HouseholdId {
        self.household_id
    }

    /// Pseudo de l'utilisateur.
    #[must_use]
    pub fn username(&self) -> &Username {
        &self.username
    }

    /// Hash du mot de passe (chaîne PHC Argon2id).
    #[must_use]
    pub fn password_hash(&self) -> &PasswordHash {
        &self.password_hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepte_un_pseudo_valide() {
        assert_eq!(Username::new("  robin_33 ").unwrap().as_str(), "robin_33");
    }

    #[test]
    fn rejette_un_pseudo_vide() {
        assert_eq!(Username::new("   "), Err(UsernameError::Empty));
    }

    #[test]
    fn rejette_un_pseudo_trop_court() {
        assert_eq!(
            Username::new("a"),
            Err(UsernameError::TooShort {
                min: MIN_USERNAME_LEN
            })
        );
    }

    #[test]
    fn rejette_un_pseudo_trop_long() {
        let raw = "a".repeat(MAX_USERNAME_LEN + 1);
        assert!(matches!(
            Username::new(raw),
            Err(UsernameError::TooLong { .. })
        ));
    }

    #[test]
    fn rejette_un_caractere_invalide() {
        assert_eq!(Username::new("ro@bin"), Err(UsernameError::InvalidChar));
    }
}
