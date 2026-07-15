//! Erreur partagée par les traits de repository des domaines.

/// Échec d'une opération de persistance, exposée par les traits de repository
/// définis dans les domaines. Le domaine reste agnostique du backend : une
/// panne technique concrète (SQLx…) est convertie en [`RepositoryError::Backend`]
/// par la couche infrastructure.
#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    /// L'entité demandée n'existe pas (ou n'appartient pas au foyer courant).
    #[error("entity not found")]
    NotFound,
    /// Panne technique du backend de persistance.
    #[error("repository backend error: {0}")]
    Backend(String),
}
