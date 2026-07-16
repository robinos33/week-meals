//! Couche infrastructure de `auth` : implémentations SQLx des ports déclarés
//! dans [`domain::repository`](crate::domain::repository).
//!
//! Les requêtes sont **exécutées au runtime** (`sqlx::query_as`), sans macros
//! vérifiées à la compilation : le workspace se compile donc sans base de
//! données disponible (la vérification se fait par les tests d'intégration
//! Testcontainers). Toute erreur SQLx est traduite en
//! [`RepositoryError::Backend`].

use kernel::{HouseholdId, RepositoryError, UserId};
use sqlx::PgPool;
use uuid::Uuid;

use crate::domain::household::{Household, HouseholdName};
use crate::domain::password::PasswordHash;
use crate::domain::repository::{HouseholdRepository, UserRepository};
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
    password_hash: String,
}

impl UserRow {
    fn into_domain(self) -> Result<User, RepositoryError> {
        let username = Username::new(self.username)
            .map_err(|e| RepositoryError::Backend(format!("invalid stored username: {e}")))?;
        Ok(User::from_parts(
            UserId::from(self.id),
            HouseholdId::from(self.household_id),
            username,
            PasswordHash::from_phc(self.password_hash),
        ))
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
        sqlx::query(
            "insert into users (id, household_id, username, password_hash) \
             values ($1, $2, $3, $4)",
        )
        .bind(user.id.as_uuid())
        .bind(user.household_id.as_uuid())
        .bind(user.username.as_str())
        .bind(user.password_hash.as_str())
        .execute(&self.pool)
        .await
        .map_err(backend)?;
        Ok(())
    }

    async fn find(&self, id: UserId) -> Result<Option<User>, RepositoryError> {
        let row: Option<UserRow> = sqlx::query_as(
            "select id, household_id, username, password_hash from users where id = $1",
        )
        .bind(id.as_uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(backend)?;
        row.map(UserRow::into_domain).transpose()
    }

    async fn find_by_username(&self, username: &Username) -> Result<Option<User>, RepositoryError> {
        let row: Option<UserRow> = sqlx::query_as(
            "select id, household_id, username, password_hash from users where username = $1",
        )
        .bind(username.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(backend)?;
        row.map(UserRow::into_domain).transpose()
    }
}
