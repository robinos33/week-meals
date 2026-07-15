//! Couche domaine de `auth` : entités, value objects, service de hachage et
//! traits de repository. Aucune dépendance à SQLx/Axum (règle de convention,
//! cf. ADR-0005).
//!
//! - [`household`]   — foyer ([`Household`]) et son nom validé
//! - [`user`]        — utilisateur ([`User`]) et pseudo validé ([`Username`])
//! - [`password`]    — VO mot de passe et service de hachage **Argon2id** *pur*
//! - [`repository`]  — ports de persistance ([`HouseholdRepository`],
//!   [`UserRepository`]), implémentés en infrastructure
//!
//! Toutes les entités sont scopées à un foyer ([`kernel::HouseholdId`]) :
//! l'application est multi-foyers *by design* (cf. ADR-0002).

pub mod household;
pub mod password;
pub mod repository;
pub mod user;

pub use household::{Household, HouseholdName, HouseholdNameError};
pub use password::{
    Argon2Hasher, Password, PasswordError, PasswordHash, PasswordHashError, PasswordHasher,
    MAX_PASSWORD_LEN, MIN_PASSWORD_LEN,
};
pub use repository::{HouseholdRepository, UserRepository};
pub use user::{User, Username, UsernameError, MAX_USERNAME_LEN, MIN_USERNAME_LEN};
