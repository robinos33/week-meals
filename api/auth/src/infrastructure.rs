//! Couche infrastructure de `auth` : implémentations SQLx des ports déclarés
//! dans [`domain::repository`](crate::domain::repository).
//!
//! Les requêtes sont **exécutées au runtime** (`sqlx::query_as`), sans macros
//! vérifiées à la compilation : le workspace se compile donc sans base de
//! données disponible (la vérification se fait par les tests d'intégration
//! Testcontainers). Toute erreur SQLx est traduite en
//! [`RepositoryError::Backend`].

use chrono::{DateTime, Utc};
use kernel::{DeviceId, HouseholdId, RepositoryError, UserId};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::device::{Device, DeviceLabel, OnboardingWindow};
use crate::domain::household::{Household, HouseholdName, WeekStartDay};
use crate::domain::pairing::PairingCodeHash;
use crate::domain::repository::{
    DeviceRepository, HouseholdRepository, OnboardingRepository, UserRepository,
};
use crate::domain::user::{User, Username};

/// Traduit une erreur SQLx en erreur de repository agnostique.
fn backend(err: sqlx::Error) -> RepositoryError {
    RepositoryError::Backend(err.to_string())
}

/// Ligne SQL d'un foyer.
#[derive(sqlx::FromRow)]
struct HouseholdRow {
    id: Uuid,
    name: String,
}

impl HouseholdRow {
    /// Reconstitue l'entité de domaine. Un nom invalide en base est une
    /// corruption de données → erreur backend.
    fn into_domain(self) -> Result<Household, RepositoryError> {
        let name = HouseholdName::new(self.name)
            .map_err(|e| RepositoryError::Backend(format!("invalid stored household name: {e}")))?;
        Ok(Household::from_parts(HouseholdId::from(self.id), name))
    }
}

/// Ligne SQL d'un utilisateur.
#[derive(sqlx::FromRow)]
struct UserRow {
    id: Uuid,
    household_id: Uuid,
    username: String,
}

impl UserRow {
    fn into_domain(self) -> Result<User, RepositoryError> {
        let username = Username::new(self.username)
            .map_err(|e| RepositoryError::Backend(format!("invalid stored username: {e}")))?;
        Ok(User::from_parts(
            UserId::from(self.id),
            HouseholdId::from(self.household_id),
            username,
        ))
    }
}

/// Ligne SQL d'un appareil enrôlé.
#[derive(sqlx::FromRow)]
struct DeviceRow {
    id: Uuid,
    user_id: Uuid,
    credential_id: Vec<u8>,
    passkey: serde_json::Value,
    label: String,
    backup_eligible: bool,
    backup_state: bool,
    created_at: DateTime<Utc>,
    last_seen_at: Option<DateTime<Utc>>,
}

impl DeviceRow {
    fn into_domain(self) -> Result<Device, RepositoryError> {
        let label = DeviceLabel::new(self.label)
            .map_err(|e| RepositoryError::Backend(format!("invalid stored device label: {e}")))?;
        Ok(Device {
            id: DeviceId::from(self.id),
            user_id: UserId::from(self.user_id),
            credential_id: self.credential_id,
            passkey_json: self.passkey.to_string(),
            label,
            backup_eligible: self.backup_eligible,
            backup_state: self.backup_state,
            created_at: self.created_at,
            last_seen_at: self.last_seen_at,
        })
    }
}

/// Repository SQLx des foyers.
#[derive(Clone)]
pub struct SqlxHouseholdRepository {
    pool: PgPool,
}

impl SqlxHouseholdRepository {
    /// Construit le repository à partir d'un pool Postgres.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl HouseholdRepository for SqlxHouseholdRepository {
    async fn create(&self, household: &Household) -> Result<(), RepositoryError> {
        sqlx::query("insert into households (id, name) values ($1, $2)")
            .bind(household.id.as_uuid())
            .bind(household.name.as_str())
            .execute(&self.pool)
            .await
            .map_err(backend)?;
        Ok(())
    }

    async fn find(&self, id: HouseholdId) -> Result<Option<Household>, RepositoryError> {
        let row: Option<HouseholdRow> =
            sqlx::query_as("select id, name from households where id = $1")
                .bind(id.as_uuid())
                .fetch_optional(&self.pool)
                .await
                .map_err(backend)?;
        row.map(HouseholdRow::into_domain).transpose()
    }

    async fn week_start_day(&self, id: HouseholdId) -> Result<WeekStartDay, RepositoryError> {
        let row: Option<(i16,)> =
            sqlx::query_as("select week_start_day from households where id = $1")
                .bind(id.as_uuid())
                .fetch_optional(&self.pool)
                .await
                .map_err(backend)?;
        let (raw,) = row.ok_or(RepositoryError::NotFound)?;
        // La contrainte `check (0..6)` en base garantit la plage ; un écart
        // serait une corruption de données.
        u8::try_from(raw)
            .ok()
            .and_then(|v| WeekStartDay::new(v).ok())
            .ok_or_else(|| {
                RepositoryError::Backend(format!("invalid stored week_start_day: {raw}"))
            })
    }

    async fn set_week_start_day(
        &self,
        id: HouseholdId,
        day: WeekStartDay,
    ) -> Result<(), RepositoryError> {
        let result = sqlx::query("update households set week_start_day = $2 where id = $1")
            .bind(id.as_uuid())
            .bind(i16::from(day.value()))
            .execute(&self.pool)
            .await
            .map_err(backend)?;
        if result.rows_affected() == 0 {
            Err(RepositoryError::NotFound)
        } else {
            Ok(())
        }
    }
}

/// Repository SQLx des utilisateurs.
#[derive(Clone)]
pub struct SqlxUserRepository {
    pool: PgPool,
}

impl SqlxUserRepository {
    /// Construit le repository à partir d'un pool Postgres.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait::async_trait]
impl UserRepository for SqlxUserRepository {
    async fn create(&self, user: &User) -> Result<(), RepositoryError> {
        sqlx::query("insert into users (id, household_id, username) values ($1, $2, $3)")
            .bind(user.id.as_uuid())
            .bind(user.household_id.as_uuid())
            .bind(user.username.as_str())
            .execute(&self.pool)
            .await
            .map_err(backend)?;
        Ok(())
    }

    async fn find(&self, id: UserId) -> Result<Option<User>, RepositoryError> {
        let row: Option<UserRow> =
            sqlx::query_as("select id, household_id, username from users where id = $1")
                .bind(id.as_uuid())
                .fetch_optional(&self.pool)
                .await
                .map_err(backend)?;
        row.map(UserRow::into_domain).transpose()
    }

    async fn find_by_username(&self, username: &Username) -> Result<Option<User>, RepositoryError> {
        let row: Option<UserRow> = sqlx::query_as(
            "select id, household_id, username from users \
             where username = $1 order by created_at limit 1",
        )
        .bind(username.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(backend)?;
        row.map(UserRow::into_domain).transpose()
    }
}

/// Repository SQLx des appareils enrôlés.
#[derive(Clone)]
pub struct SqlxDeviceRepository {
    pool: PgPool,
}

impl SqlxDeviceRepository {
    /// Construit le repository à partir d'un pool Postgres.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// Colonnes d'un appareil, réutilisées par les `select`.
const DEVICE_COLUMNS: &str = "id, user_id, credential_id, passkey, label, \
     backup_eligible, backup_state, created_at, last_seen_at";

#[async_trait::async_trait]
impl DeviceRepository for SqlxDeviceRepository {
    async fn create(&self, device: &Device) -> Result<(), RepositoryError> {
        let passkey: serde_json::Value = serde_json::from_str(&device.passkey_json)
            .map_err(|e| RepositoryError::Backend(format!("passkey JSON invalide : {e}")))?;
        sqlx::query(
            "insert into devices \
             (id, user_id, credential_id, passkey, label, backup_eligible, backup_state) \
             values ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(device.id.as_uuid())
        .bind(device.user_id.as_uuid())
        .bind(&device.credential_id)
        .bind(passkey)
        .bind(device.label.as_str())
        .bind(device.backup_eligible)
        .bind(device.backup_state)
        .execute(&self.pool)
        .await
        .map_err(backend)?;
        Ok(())
    }

    async fn list_by_user(&self, user_id: UserId) -> Result<Vec<Device>, RepositoryError> {
        let rows: Vec<DeviceRow> = sqlx::query_as(&format!(
            "select {DEVICE_COLUMNS} from devices where user_id = $1 order by created_at"
        ))
        .bind(user_id.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(backend)?;
        rows.into_iter().map(DeviceRow::into_domain).collect()
    }

    async fn list_by_household(
        &self,
        household_id: HouseholdId,
    ) -> Result<Vec<Device>, RepositoryError> {
        let rows: Vec<DeviceRow> = sqlx::query_as(&format!(
            "select {} from devices d join users u on u.id = d.user_id \
             where u.household_id = $1 order by d.created_at",
            DEVICE_COLUMNS
                .split(", ")
                .map(|c| format!("d.{c}"))
                .collect::<Vec<_>>()
                .join(", ")
        ))
        .bind(household_id.as_uuid())
        .fetch_all(&self.pool)
        .await
        .map_err(backend)?;
        rows.into_iter().map(DeviceRow::into_domain).collect()
    }

    async fn find_by_credential(
        &self,
        credential_id: &[u8],
    ) -> Result<Option<Device>, RepositoryError> {
        let row: Option<DeviceRow> = sqlx::query_as(&format!(
            "select {DEVICE_COLUMNS} from devices where credential_id = $1"
        ))
        .bind(credential_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(backend)?;
        row.map(DeviceRow::into_domain).transpose()
    }

    async fn update_after_auth(
        &self,
        credential_id: &[u8],
        passkey_json: &str,
        backup_eligible: bool,
        backup_state: bool,
        last_seen_at: DateTime<Utc>,
    ) -> Result<(), RepositoryError> {
        let passkey: serde_json::Value = serde_json::from_str(passkey_json)
            .map_err(|e| RepositoryError::Backend(format!("passkey JSON invalide : {e}")))?;
        sqlx::query(
            "update devices set passkey = $2, backup_eligible = $3, backup_state = $4, \
             last_seen_at = $5 where credential_id = $1",
        )
        .bind(credential_id)
        .bind(passkey)
        .bind(backup_eligible)
        .bind(backup_state)
        .bind(last_seen_at)
        .execute(&self.pool)
        .await
        .map_err(backend)?;
        Ok(())
    }

    async fn revoke(
        &self,
        id: DeviceId,
        household_id: HouseholdId,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query(
            "delete from devices d using users u \
             where d.user_id = u.id and d.id = $1 and u.household_id = $2",
        )
        .bind(id.as_uuid())
        .bind(household_id.as_uuid())
        .execute(&self.pool)
        .await
        .map_err(backend)?;
        Ok(result.rows_affected() > 0)
    }
}

/// Repository SQLx de la fenêtre d'enrôlement (colonnes `households.onboarding_*`).
#[derive(Clone)]
pub struct SqlxOnboardingRepository {
    pool: PgPool,
}

impl SqlxOnboardingRepository {
    /// Construit le repository à partir d'un pool Postgres.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// Ligne SQL de la fenêtre d'enrôlement.
#[derive(sqlx::FromRow)]
struct OnboardingRow {
    onboarding_until: Option<DateTime<Utc>>,
    onboarding_code_hash: Option<String>,
    onboarding_attempts: i32,
    onboarding_user_id: Option<Uuid>,
}

#[async_trait::async_trait]
impl OnboardingRepository for SqlxOnboardingRepository {
    async fn open(
        &self,
        household_id: HouseholdId,
        until: DateTime<Utc>,
        code_hash: &PairingCodeHash,
        target_user: Option<UserId>,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "update households set onboarding_until = $2, onboarding_code_hash = $3, \
             onboarding_user_id = $4, onboarding_attempts = 0 where id = $1",
        )
        .bind(household_id.as_uuid())
        .bind(until)
        .bind(code_hash.as_str())
        .bind(target_user.map(|u| u.as_uuid()))
        .execute(&self.pool)
        .await
        .map_err(backend)?;
        Ok(())
    }

    async fn close(&self, household_id: HouseholdId) -> Result<(), RepositoryError> {
        sqlx::query(
            "update households set onboarding_until = null, onboarding_code_hash = null, \
             onboarding_user_id = null, onboarding_attempts = 0 where id = $1",
        )
        .bind(household_id.as_uuid())
        .execute(&self.pool)
        .await
        .map_err(backend)?;
        Ok(())
    }

    async fn get(
        &self,
        household_id: HouseholdId,
    ) -> Result<Option<OnboardingWindow>, RepositoryError> {
        let row: Option<OnboardingRow> = sqlx::query_as(
            "select onboarding_until, onboarding_code_hash, onboarding_attempts, \
             onboarding_user_id from households where id = $1",
        )
        .bind(household_id.as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(backend)?;

        Ok(row.and_then(|r| {
            match (r.onboarding_until, r.onboarding_code_hash) {
                (Some(until), Some(hash)) => Some(OnboardingWindow {
                    until,
                    code_hash: PairingCodeHash::from_phc(hash),
                    attempts: r.onboarding_attempts,
                    target_user: r.onboarding_user_id.map(UserId::from),
                }),
                // Colonnes partiellement nulles ⇒ pas de fenêtre.
                _ => None,
            }
        }))
    }

    async fn record_failure(&self, household_id: HouseholdId) -> Result<i32, RepositoryError> {
        let row: (i32,) = sqlx::query_as(
            "update households set onboarding_attempts = onboarding_attempts + 1 \
             where id = $1 returning onboarding_attempts",
        )
        .bind(household_id.as_uuid())
        .fetch_one(&self.pool)
        .await
        .map_err(backend)?;
        Ok(row.0)
    }
}
