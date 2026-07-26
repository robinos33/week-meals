//! Utilitaires de test partagés : un [`ShoppingListRepository`] en mémoire pour
//! exercer les use cases sans base. Compilé uniquement pour les tests.

use std::sync::Mutex;

use kernel::{HouseholdId, RepositoryError, ShoppingItemId};

use crate::domain::{ShoppingItem, ShoppingListRepository};

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
