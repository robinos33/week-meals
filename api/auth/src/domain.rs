//! Couche domaine de `auth` : entités, value objects, traits de repository
//! et services purs. Aucune dépendance à SQLx/Axum (règle de convention).
//!
//! Modélise l'authentification par **passkeys** (cf. [ADR-0006]) :
//!
//! - [`household`]  — le foyer ([`Household`], [`HouseholdName`]).
//! - [`user`]       — un membre du foyer ([`User`], [`Username`]) : un pseudo
//!   d'affichage, plus aucun secret.
//! - [`device`]     — un appareil enrôlé porteur d'une passkey ([`Device`],
//!   [`DeviceLabel`]) et la fenêtre d'enrôlement ([`OnboardingWindow`]).
//! - [`pairing`]    — code d'appairage à usage unique + hachage Argon2id
//!   ([`PairingCode`], [`PairingCodeHash`], port [`PairingHasher`]).
//! - [`repository`] — ports de persistance ([`HouseholdRepository`],
//!   [`UserRepository`], [`DeviceRepository`], [`OnboardingRepository`]).
//!
//! [ADR-0006]: ../../../docs/adr/0006-auth-passkeys-appareils-enroles.md

pub mod device;
pub mod household;
pub mod pairing;
pub mod repository;
pub mod user;

pub use device::{Device, DeviceLabel, OnboardingWindow, MAX_ONBOARDING_ATTEMPTS};
pub use household::{Household, HouseholdName};
pub use pairing::{Argon2PairingHasher, PairingCode, PairingCodeHash, PairingError, PairingHasher};
pub use repository::{DeviceRepository, HouseholdRepository, OnboardingRepository, UserRepository};
pub use user::{User, Username};

/// Violation d'un invariant du domaine `auth`.
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
    /// Le libellé d'appareil ne peut pas être vide.
    #[error("device label must not be empty")]
    EmptyDeviceLabel,
    /// Le libellé d'appareil dépasse la longueur maximale autorisée.
    #[error("device label must be at most {max} characters")]
    DeviceLabelTooLong {
        /// Longueur maximale autorisée.
        max: usize,
    },
}
