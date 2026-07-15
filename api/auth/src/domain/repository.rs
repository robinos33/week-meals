//! Ports de persistance de `auth` : traits déclarés ici (domaine), implémentés
//! en couche infrastructure (repos SQLx). Toutes les opérations sont scopées au
//! foyer.

use async_trait::async_trait;
use kernel::{HouseholdId, RepositoryError, UserId};

use super::household::Household;
use super::user::{User, Username};

/// Persistance des foyers ([`Household`]).
#[async_trait]
pub trait HouseholdRepository: Send + Sync {
    /// Persiste un nouveau foyer.
    ///
    /// # Errors
    ///
    /// Renvoie [`RepositoryError`] en cas de conflit ou de défaillance du
    /// backend.
    async fn create(&self, household: &Household) -> Result<(), RepositoryError>;

    /// Récupère un foyer par son identifiant, ou `None` s'il n'existe pas.
    ///
    /// # Errors
    ///
    /// Renvoie [`RepositoryError`] en cas de défaillance du backend.
    async fn find_by_id(&self, id: HouseholdId) -> Result<Option<Household>, RepositoryError>;
}

/// Persistance des utilisateurs ([`User`]).
#[async_trait]
pub trait UserRepository: Send + Sync {
    /// Persiste un nouvel utilisateur.
    ///
    /// # Errors
    ///
    /// Renvoie [`RepositoryError::Conflict`] si le pseudo est déjà pris dans le
    /// foyer, ou une autre variante en cas de défaillance du backend.
    async fn create(&self, user: &User) -> Result<(), RepositoryError>;

    /// Récupère un utilisateur par son identifiant, ou `None`.
    ///
    /// # Errors
    ///
    /// Renvoie [`RepositoryError`] en cas de défaillance du backend.
    async fn find_by_id(&self, id: UserId) -> Result<Option<User>, RepositoryError>;

    /// Récupère un utilisateur par son pseudo **au sein d'un foyer**, ou `None`.
    ///
    /// Le pseudo n'est unique que dans le périmètre du foyer : deux foyers
    /// distincts peuvent héberger le même pseudo (multi-foyers *by design*).
    ///
    /// # Errors
    ///
    /// Renvoie [`RepositoryError`] en cas de défaillance du backend.
    async fn find_by_username(
        &self,
        household_id: HouseholdId,
        username: &Username,
    ) -> Result<Option<User>, RepositoryError>;
}
