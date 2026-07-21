//! Ports de persistance du domaine `auth`. Déclarés ici (domaine), implémentés
//! en `infrastructure` (SQLx). Le domaine ne connaît que ces traits.

use chrono::{DateTime, Utc};
use kernel::{DeviceId, HouseholdId, RepositoryError, UserId};

use super::device::{Device, OnboardingWindow};
use super::household::{Household, WeekStartDay};
use super::pairing::PairingCodeHash;
use super::user::{User, Username};

/// Persistance des foyers.
#[async_trait::async_trait]
pub trait HouseholdRepository: Send + Sync {
    /// Persiste un nouveau foyer.
    async fn create(&self, household: &Household) -> Result<(), RepositoryError>;

    /// Récupère un foyer par identifiant, s'il existe.
    async fn find(&self, id: HouseholdId) -> Result<Option<Household>, RepositoryError>;

    /// Lit le premier jour de la semaine du foyer (#57).
    ///
    /// # Errors
    /// [`RepositoryError::NotFound`] si le foyer n'existe pas.
    async fn week_start_day(&self, id: HouseholdId) -> Result<WeekStartDay, RepositoryError>;

    /// Fixe le premier jour de la semaine du foyer (#57).
    ///
    /// # Errors
    /// [`RepositoryError::NotFound`] si le foyer n'existe pas.
    async fn set_week_start_day(
        &self,
        id: HouseholdId,
        day: WeekStartDay,
    ) -> Result<(), RepositoryError>;
}

/// Persistance des utilisateurs.
#[async_trait::async_trait]
pub trait UserRepository: Send + Sync {
    /// Persiste un nouvel utilisateur.
    async fn create(&self, user: &User) -> Result<(), RepositoryError>;

    /// Récupère un utilisateur par identifiant, s'il existe.
    async fn find(&self, id: UserId) -> Result<Option<User>, RepositoryError>;

    /// Récupère un utilisateur par pseudo (le plus ancien en cas d'homonymie).
    /// Sert au CLI `device open-window --for <pseudo>`.
    async fn find_by_username(&self, username: &Username) -> Result<Option<User>, RepositoryError>;
}

/// Persistance des appareils enrôlés (passkeys).
#[async_trait::async_trait]
pub trait DeviceRepository: Send + Sync {
    /// Enrôle un appareil (insère la passkey).
    async fn create(&self, device: &Device) -> Result<(), RepositoryError>;

    /// Liste les appareils d'un utilisateur (pour l'authentification découvrable).
    async fn list_by_user(&self, user_id: UserId) -> Result<Vec<Device>, RepositoryError>;

    /// Liste les appareils d'un foyer (pour la carte Appareils des réglages).
    async fn list_by_household(
        &self,
        household_id: HouseholdId,
    ) -> Result<Vec<Device>, RepositoryError>;

    /// Retrouve un appareil par identifiant de credential WebAuthn.
    async fn find_by_credential(
        &self,
        credential_id: &[u8],
    ) -> Result<Option<Device>, RepositoryError>;

    /// Met à jour la passkey après une authentification réussie (compteur de
    /// signature, drapeau de sauvegarde, dernière activité).
    async fn update_after_auth(
        &self,
        credential_id: &[u8],
        passkey_json: &str,
        backup_eligible: bool,
        backup_state: bool,
        last_seen_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError>;

    /// Révoque un appareil du foyer donné. Renvoie `true` si une ligne a été
    /// supprimée (l'appareil appartenait bien au foyer).
    async fn revoke(
        &self,
        id: DeviceId,
        household_id: HouseholdId,
    ) -> Result<bool, RepositoryError>;
}

/// Pilotage de la fenêtre d'enrôlement (colonnes `households.onboarding_*`).
#[async_trait::async_trait]
pub trait OnboardingRepository: Send + Sync {
    /// Ouvre (ou remplace) la fenêtre : fixe l'expiration, le hash du code et la
    /// cible, et remet le compteur de tentatives à zéro.
    async fn open(
        &self,
        household_id: HouseholdId,
        until: DateTime<Utc>,
        code_hash: &PairingCodeHash,
        target_user: Option<UserId>,
    ) -> Result<(), RepositoryError>;

    /// Ferme la fenêtre (efface expiration, hash, cible, tentatives).
    async fn close(&self, household_id: HouseholdId) -> Result<(), RepositoryError>;

    /// Lit la fenêtre courante, ou `None` si aucune n'est ouverte en base.
    async fn get(
        &self,
        household_id: HouseholdId,
    ) -> Result<Option<OnboardingWindow>, RepositoryError>;

    /// Enregistre un échec de code et renvoie le nouveau nombre de tentatives.
    async fn record_failure(&self, household_id: HouseholdId) -> Result<i32, RepositoryError>;
}
