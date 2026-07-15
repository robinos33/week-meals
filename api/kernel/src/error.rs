//! Erreurs partagées entre domaines.

use thiserror::Error;

/// Erreur renvoyée par les traits de repository (déclarés dans les couches
/// `domain`).
///
/// Les implémentations concrètes (couche infrastructure) traduisent leurs
/// erreurs techniques (SQLx, réseau…) vers ces variantes : ainsi le domaine et
/// l'application n'ont aucune dépendance vers un backend de persistance donné.
#[derive(Debug, Error)]
pub enum RepositoryError {
    /// Violation d'une contrainte d'unicité (p. ex. pseudo déjà pris dans le
    /// foyer).
    #[error("conflit d'unicité : {0}")]
    Conflict(String),

    /// Défaillance technique du backend (connexion, requête, sérialisation…).
    #[error("erreur du backend de persistance : {0}")]
    Backend(String),
}
