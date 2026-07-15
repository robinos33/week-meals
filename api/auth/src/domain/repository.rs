//! Ports de persistance du domaine `auth`. Déclarés ici (domaine), implémentés
//! en `infrastructure` (SQLx). Le domaine ne connaît que ces traits.

use kernel::{HouseholdId, RepositoryError, UserId};

use super::household::Household;
use super::user::{User, Username};

/// Persistance des foyers.
#[async_trait::async_trait]
pub trait HouseholdRepository: Send + Sync {
    /// Persiste un nouveau foyer.
    async fn create(&self, household: &Household) -> Result<(), RepositoryError>;

    /// Récupère un foyer par identifiant, s'il existe.
    async fn find(&self, id: HouseholdId) -> Result<Option<Household>, RepositoryError>;
}

/// Persistance des utilisateurs.
#[async_trait::async_trait]
pub trait UserRepository: Send + Sync {
    /// Persiste un nouvel utilisateur.
    async fn create(&self, user: &User) -> Result<(), RepositoryError>;

    /// Récupère un utilisateur par identifiant, s'il existe.
    async fn find(&self, id: UserId) -> Result<Option<User>, RepositoryError>;

    /// Récupère un utilisateur par pseudo (unique globalement) pour le login.
    async fn find_by_username(&self, username: &Username) -> Result<Option<User>, RepositoryError>;
}
