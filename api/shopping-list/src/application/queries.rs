//! Use cases de lecture : la liste de courses courante du foyer, le
//! dictionnaire d'ingrédients qui sert l'auto-complétion de la saisie, et les
//! magasins du foyer (ordre de visite des rayons).

use kernel::HouseholdId;

use crate::domain::{
    IngredientReference, ReferenceRepository, ShoppingItem, ShoppingListRepository, Store,
    StoreRepository,
};

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

/// Résultat d'une lecture du dictionnaire.
#[derive(Debug)]
pub enum GetDictionaryResponse {
    /// Entrées du dictionnaire, par nom canonique.
    Loaded(Vec<IngredientReference>),
    /// Panne technique.
    Unavailable,
}

/// Handler de la lecture du dictionnaire.
///
/// Le dictionnaire est **global** (pas de scope foyer) et change rarement :
/// le front le charge une fois et le garde en cache pour proposer ses
/// suggestions de saisie.
pub struct GetDictionaryHandler<'a> {
    references: &'a dyn ReferenceRepository,
}

impl<'a> GetDictionaryHandler<'a> {
    /// Construit le handler.
    #[must_use]
    pub fn new(references: &'a dyn ReferenceRepository) -> Self {
        Self { references }
    }

    /// Exécute la lecture. Ne renvoie jamais d'erreur.
    pub async fn handle(&self) -> GetDictionaryResponse {
        match self.references.catalog().await {
            Ok(catalog) => GetDictionaryResponse::Loaded(catalog.entries().to_vec()),
            Err(_) => GetDictionaryResponse::Unavailable,
        }
    }
}

/// Query : les magasins d'un foyer.
#[derive(Debug, Clone)]
pub struct GetStoresQuery {
    /// Foyer propriétaire (scope).
    pub household_id: HouseholdId,
}

/// Résultat d'une lecture des magasins.
#[derive(Debug)]
pub enum GetStoresResponse {
    /// Magasins du foyer, avec l'ordre de visite de leurs rayons.
    Loaded(Vec<Store>),
    /// Panne technique.
    Unavailable,
}

/// Handler de la lecture des magasins.
pub struct GetStoresHandler<'a> {
    stores: &'a dyn StoreRepository,
}

impl<'a> GetStoresHandler<'a> {
    /// Construit le handler.
    #[must_use]
    pub fn new(stores: &'a dyn StoreRepository) -> Self {
        Self { stores }
    }

    /// Exécute la lecture. Ne renvoie jamais d'erreur.
    pub async fn handle(&self, query: GetStoresQuery) -> GetStoresResponse {
        match self.stores.list(query.household_id).await {
            Ok(stores) => GetStoresResponse::Loaded(stores),
            Err(_) => GetStoresResponse::Unavailable,
        }
    }
}
