//! Use case de lecture : la liste de courses courante du foyer.

use kernel::HouseholdId;

use crate::domain::{ShoppingItem, ShoppingListRepository};

/// Query : la liste courante du foyer.
#[derive(Debug, Clone)]
pub struct GetListQuery {
    /// Foyer propriétaire (scope).
    pub household_id: HouseholdId,
}

/// Résultat d'une lecture de liste.
#[derive(Debug)]
pub enum GetListResponse {
    /// Lignes de la liste (ordre défini par le repo).
    Loaded(Vec<ShoppingItem>),
    /// Panne technique.
    Unavailable,
}

/// Handler de la lecture.
pub struct GetListHandler<'a> {
    items: &'a dyn ShoppingListRepository,
}

impl<'a> GetListHandler<'a> {
    /// Construit le handler.
    #[must_use]
    pub fn new(items: &'a dyn ShoppingListRepository) -> Self {
        Self { items }
    }

    /// Exécute la lecture. Ne renvoie jamais d'erreur.
    pub async fn handle(&self, query: GetListQuery) -> GetListResponse {
        match self.items.list(query.household_id).await {
            Ok(items) => GetListResponse::Loaded(items),
            Err(_) => GetListResponse::Unavailable,
        }
    }
}
