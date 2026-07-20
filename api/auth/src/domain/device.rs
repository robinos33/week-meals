//! Appareils enrôlés et fenêtre d'enrôlement (cf. ADR-0006).
//!
//! - [`Device`] : un appareil porteur d'une passkey. Le serveur ne conserve que
//!   la clé publique — ici le credential webauthn-rs sérialisé (JSON opaque),
//!   relu tel quel aux cérémonies d'authentification. `credential_id` en est
//!   extrait pour l'unicité et les recherches.
//! - [`OnboardingWindow`] : la fenêtre temporelle, bornée par un code
//!   d'appairage et un plafond de tentatives, pendant laquelle un appareil
//!   inconnu peut enrôler une passkey.

use chrono::{DateTime, Utc};
use kernel::{DeviceId, UserId};

use super::pairing::PairingCodeHash;
use super::AuthError;

/// Longueur maximale d'un libellé d'appareil.
const DEVICE_LABEL_MAX_LEN: usize = 64;

/// Nombre d'échecs de code au-delà duquel la fenêtre se referme (ADR-0006).
pub const MAX_ONBOARDING_ATTEMPTS: i32 = 5;

/// Libellé d'un appareil (« iPhone de Robin ») : chaîne non vide, bornée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceLabel(String);

impl DeviceLabel {
    /// Construit un libellé valide (trim appliqué).
    ///
    /// # Errors
    /// - [`AuthError::EmptyDeviceLabel`] si vide (après trim).
    /// - [`AuthError::DeviceLabelTooLong`] au-delà de [`DEVICE_LABEL_MAX_LEN`].
    pub fn new(value: impl Into<String>) -> Result<Self, AuthError> {
        let value = value.into().trim().to_owned();
        if value.is_empty() {
            return Err(AuthError::EmptyDeviceLabel);
        }
        if value.chars().count() > DEVICE_LABEL_MAX_LEN {
            return Err(AuthError::DeviceLabelTooLong {
                max: DEVICE_LABEL_MAX_LEN,
            });
        }
        Ok(Self(value))
    }

    /// Le libellé sous forme de `&str`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DeviceLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Un appareil enrôlé : une passkey rattachée à un utilisateur.
#[derive(Debug, Clone)]
pub struct Device {
    /// Identifiant de l'appareil.
    pub id: DeviceId,
    /// Utilisateur propriétaire.
    pub user_id: UserId,
    /// Identifiant du credential WebAuthn (octets bruts), unique.
    pub credential_id: Vec<u8>,
    /// Credential webauthn-rs sérialisé en JSON (clé publique COSE, compteur,
    /// AAGUID…). Opaque au domaine : relu tel quel par la présentation.
    pub passkey_json: String,
    /// Libellé lisible.
    pub label: DeviceLabel,
    /// Le credential est éligible à la sauvegarde (synchronisable).
    pub backup_eligible: bool,
    /// Le credential est actuellement sauvegardé (synchronisé).
    pub backup_state: bool,
    /// Date d'enrôlement.
    pub created_at: DateTime<Utc>,
    /// Dernière authentification observée.
    pub last_seen_at: Option<DateTime<Utc>>,
}

/// Fenêtre d'enrôlement d'un foyer. Reconstituée depuis les colonnes
/// `households.onboarding_*`.
#[derive(Debug, Clone)]
pub struct OnboardingWindow {
    /// Instant de fermeture automatique.
    pub until: DateTime<Utc>,
    /// Hash du code d'appairage attendu.
    pub code_hash: PairingCodeHash,
    /// Nombre d'échecs de code déjà enregistrés.
    pub attempts: i32,
    /// Utilisateur cible (`--for`) ; `None` ⇒ un nouvel utilisateur est créé à
    /// la fin de la cérémonie d'enrôlement.
    pub target_user: Option<UserId>,
}

impl OnboardingWindow {
    /// La fenêtre est-elle ouverte à l'instant `now` ? Close si expirée ou si le
    /// plafond de tentatives est atteint.
    #[must_use]
    pub fn is_open(&self, now: DateTime<Utc>) -> bool {
        now < self.until && self.attempts < MAX_ONBOARDING_ATTEMPTS
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn window(attempts: i32, until: DateTime<Utc>) -> OnboardingWindow {
        OnboardingWindow {
            until,
            code_hash: PairingCodeHash::from_phc("$argon2id$stub"),
            attempts,
            target_user: None,
        }
    }

    #[test]
    fn label_trims_and_rejects_blank() {
        assert_eq!(
            DeviceLabel::new("  iPhone de Robin ").unwrap().as_str(),
            "iPhone de Robin"
        );
        assert_eq!(
            DeviceLabel::new("   ").unwrap_err(),
            AuthError::EmptyDeviceLabel
        );
    }

    #[test]
    fn open_only_before_expiry_and_under_attempt_cap() {
        let now = Utc::now();
        assert!(window(0, now + Duration::minutes(5)).is_open(now));
        // Expirée.
        assert!(!window(0, now - Duration::minutes(1)).is_open(now));
        // Plafond de tentatives atteint.
        assert!(!window(MAX_ONBOARDING_ATTEMPTS, now + Duration::minutes(5)).is_open(now));
    }
}
