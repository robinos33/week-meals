//! Couche domaine de `auth` : entités, value objects, traits de repository
//! et services purs. Aucune dépendance à SQLx/Axum (règle de convention).
//!
//! Modélise l'authentification sans email (cf. [ADR-0002]) :
//!
//! - [`household`]  — le foyer ([`Household`], [`HouseholdName`]).
//! - [`user`]       — un membre du foyer ([`User`], [`Username`]).
//! - [`password`]   — secret + hachage **Argon2id** pur ([`Password`],
//!   [`PasswordHash`], port [`PasswordHasher`], service [`Argon2Hasher`]).
//! - [`repository`] — ports de persistance ([`HouseholdRepository`],
//!   [`UserRepository`]).
//!
//! [ADR-0002]: ../../../docs/adr/0002-auth-sans-email.md

pub mod household;
pub mod password;
pub mod repository;
pub mod user;

pub use household::{Household, HouseholdName};
pub use password::{Argon2Hasher, Password, PasswordError, PasswordHash, PasswordHasher};
pub use repository::{HouseholdRepository, UserRepository};
pub use user::{User, Username};

/// Violation d'un invariant du domaine `auth` (hors hachage, cf.
/// [`PasswordError`]).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    /// Le nom de foyer ne peut pas être vide.
    #[error("household name must not be empty")]
    EmptyHouseholdName,
    /// Le pseudo ne peut pas être vide.
    #[error("username must not be empty")]
    EmptyUsername,
    /// Le pseudo dépasse la longueur maximale autorisée.
    #[error("username must be at most {max} characters")]
    UsernameTooLong {
        /// Longueur maximale autorisée.
        max: usize,
    },
    /// Le mot de passe est plus court que le minimum requis.
    #[error("password must be at least {min} characters")]
    PasswordTooShort {
        /// Longueur minimale requise.
        min: usize,
    },
}
