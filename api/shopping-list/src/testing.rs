//! Utilitaires de test partagés : un [`ShoppingListRepository`], un
//! [`ReferenceRepository`] et un [`StoreRepository`] en mémoire pour exercer
//! les use cases sans base. Compilé uniquement pour les tests.

use std::sync::Mutex;

use kernel::{HouseholdId, RepositoryError, ShoppingItemId, StoreId};

use crate::domain::{
    IngredientReference, ReferenceCatalog, ReferenceRepository, ShoppingItem,
    ShoppingListRepository, Store, StoreRepository,
};

/// Repository en mémoire, scopé au foyer comme l'implémentation SQLx.
///
/// L'ordre de [`list`](ShoppingListRepository::list) suit `position` comme la
/// production (`order by position`), pour que les use cases exercent la même
/// fusion « premier match » qu'en réel.
#[derive(Default)]
pub struct InMemoryShoppingList {
    items: Mutex<Vec<ShoppingItem>>,
}

impl InMemoryShoppingList {
    /// Repo pré-rempli.
    #[must_use]
    pub fn with(items: Vec<ShoppingItem>) -> Self {
        Self {
            items: Mutex::new(items),
        }
    }
}

/// Dictionnaire en mémoire, pour exercer le rapprochement des noms sans base.
#[derive(Default)]
pub struct InMemoryReferences {
    catalog: ReferenceCatalog,
}

impl InMemoryReferences {
    /// Dictionnaire pré-rempli.
    #[must_use]
    pub fn with(references: Vec<IngredientReference>) -> Self {
        Self {
            catalog: ReferenceCatalog::from_iter(references),
        }
    }
}

#[async_trait::async_trait]
impl ReferenceRepository for InMemoryReferences {
    async fn catalog(&self) -> Result<ReferenceCatalog, RepositoryError> {
        Ok(self.catalog.clone())
    }
}

#[async_trait::async_trait]
impl ShoppingListRepository for InMemoryShoppingList {
    async fn list(&self, household_id: HouseholdId) -> Result<Vec<ShoppingItem>, RepositoryError> {
        let mut found: Vec<ShoppingItem> = self
            .items
            .lock()
            .unwrap()
            .iter()
            .filter(|item| item.household_id == household_id)
            .cloned()
            .collect();
        found.sort_by_key(|item| item.position);
        Ok(found)
    }

    async fn find(
        &self,
        household_id: HouseholdId,
        id: ShoppingItemId,
    ) -> Result<Option<ShoppingItem>, RepositoryError> {
        Ok(self
            .items
            .lock()
            .unwrap()
            .iter()
            .find(|item| item.id == id && item.household_id == household_id)
            .cloned())
    }

    async fn replace_generated(
        &self,
        household_id: HouseholdId,
        items: &[ShoppingItem],
    ) -> Result<(), RepositoryError> {
        let mut store = self.items.lock().unwrap();
        store.retain(|item| !(item.household_id == household_id && item.generated));
        store.extend(items.iter().cloned());
        Ok(())
    }

    async fn add(&self, item: &ShoppingItem) -> Result<(), RepositoryError> {
        self.items.lock().unwrap().push(item.clone());
        Ok(())
    }

    async fn update(&self, item: &ShoppingItem) -> Result<(), RepositoryError> {
        let mut store = self.items.lock().unwrap();
        match store
            .iter_mut()
            .find(|slot| slot.id == item.id && slot.household_id == item.household_id)
        {
            Some(slot) => {
                *slot = item.clone();
                Ok(())
            }
            None => Err(RepositoryError::NotFound),
        }
    }

    async fn delete(
        &self,
        household_id: HouseholdId,
        id: ShoppingItemId,
    ) -> Result<(), RepositoryError> {
        let mut store = self.items.lock().unwrap();
        let before = store.len();
        store.retain(|item| !(item.id == id && item.household_id == household_id));
        if store.len() == before {
            Err(RepositoryError::NotFound)
        } else {
            Ok(())
        }
    }

    async fn clear_checked(&self, household_id: HouseholdId) -> Result<u64, RepositoryError> {
        let mut store = self.items.lock().unwrap();
        let before = store.len();
        store.retain(|item| !(item.household_id == household_id && item.checked));
        Ok((before - store.len()) as u64)
    }

    async fn reorder(
        &self,
        household_id: HouseholdId,
        ordered_ids: &[ShoppingItemId],
    ) -> Result<(), RepositoryError> {
        let mut store = self.items.lock().unwrap();
        for (rank, id) in ordered_ids.iter().enumerate() {
            if let Some(item) = store
                .iter_mut()
                .find(|item| &item.id == id && item.household_id == household_id)
            {
                item.position = i32::try_from(rank).unwrap_or(i32::MAX);
            }
        }
        Ok(())
    }
}

/// Magasins en mémoire, scopés au foyer comme l'implémentation SQLx.
#[derive(Default)]
pub struct InMemoryStores {
    stores: Mutex<Vec<Store>>,
}

#[async_trait::async_trait]
impl StoreRepository for InMemoryStores {
    async fn list(&self, household_id: HouseholdId) -> Result<Vec<Store>, RepositoryError> {
        let mut found: Vec<Store> = self
            .stores
            .lock()
            .unwrap()
            .iter()
            .filter(|store| store.household_id == household_id)
            .cloned()
            .collect();
        found.sort_by(|left, right| {
            left.position
                .cmp(&right.position)
                .then_with(|| left.name.cmp(&right.name))
        });
        Ok(found)
    }

    async fn find(
        &self,
        household_id: HouseholdId,
        id: StoreId,
    ) -> Result<Option<Store>, RepositoryError> {
        Ok(self
            .stores
            .lock()
            .unwrap()
            .iter()
            .find(|store| store.id == id && store.household_id == household_id)
            .cloned())
    }

    async fn save(&self, store: &Store) -> Result<(), RepositoryError> {
        let mut slots = self.stores.lock().unwrap();
        match slots
            .iter_mut()
            .find(|slot| slot.id == store.id && slot.household_id == store.household_id)
        {
            Some(slot) => *slot = store.clone(),
            None => slots.push(store.clone()),
        }
        Ok(())
    }

    async fn delete(&self, household_id: HouseholdId, id: StoreId) -> Result<(), RepositoryError> {
        let mut slots = self.stores.lock().unwrap();
        let before = slots.len();
        slots.retain(|store| !(store.id == id && store.household_id == household_id));
        if slots.len() == before {
            Err(RepositoryError::NotFound)
        } else {
            Ok(())
        }
    }
}
