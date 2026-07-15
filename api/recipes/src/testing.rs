//! Utilitaires de test partagés : un [`RecipeRepository`] en mémoire pour
//! exercer les use cases sans base. Compilé uniquement pour les tests.

use std::sync::Mutex;

use kernel::{HouseholdId, Quantity, RecipeId, RepositoryError, Unit};

use crate::domain::{Recipe, RecipeIngredient, RecipeRepository};

/// Construit une recette d'exemple à un titre donné, scopée à un foyer.
pub fn sample_recipe(household_id: HouseholdId, title: &str) -> Recipe {
    let ingredient =
        RecipeIngredient::new("courgette", Quantity::new(600.0, Unit::G).unwrap()).unwrap();
    Recipe::new(
        household_id,
        title,
        Some(10),
        Some(20),
        None,
        vec![ingredient],
        vec!["Émincer.".to_owned()],
    )
    .unwrap()
}

/// Repository en mémoire, scopé au foyer comme l'implémentation SQLx.
#[derive(Default)]
pub struct InMemoryRecipes {
    recipes: Mutex<Vec<Recipe>>,
}

impl InMemoryRecipes {
    /// Repo pré-rempli.
    #[must_use]
    pub fn with(recipes: Vec<Recipe>) -> Self {
        Self {
            recipes: Mutex::new(recipes),
        }
    }

    /// Nombre de recettes stockées.
    #[must_use]
    pub fn count(&self) -> usize {
        self.recipes.lock().unwrap().len()
    }
}

#[async_trait::async_trait]
impl RecipeRepository for InMemoryRecipes {
    async fn create(&self, recipe: &Recipe) -> Result<(), RepositoryError> {
        self.recipes.lock().unwrap().push(recipe.clone());
        Ok(())
    }

    async fn find(
        &self,
        household_id: HouseholdId,
        id: RecipeId,
    ) -> Result<Option<Recipe>, RepositoryError> {
        Ok(self
            .recipes
            .lock()
            .unwrap()
            .iter()
            .find(|r| r.id == id && r.household_id == household_id)
            .cloned())
    }

    async fn list(&self, household_id: HouseholdId) -> Result<Vec<Recipe>, RepositoryError> {
        Ok(self
            .recipes
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.household_id == household_id)
            .cloned()
            .collect())
    }

    async fn search(
        &self,
        household_id: HouseholdId,
        query: &str,
    ) -> Result<Vec<Recipe>, RepositoryError> {
        let needle = query.to_lowercase();
        Ok(self
            .recipes
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.household_id == household_id && r.title.to_lowercase().contains(&needle))
            .cloned()
            .collect())
    }

    async fn update(&self, recipe: &Recipe) -> Result<(), RepositoryError> {
        let mut recipes = self.recipes.lock().unwrap();
        match recipes
            .iter_mut()
            .find(|r| r.id == recipe.id && r.household_id == recipe.household_id)
        {
            Some(slot) => {
                *slot = recipe.clone();
                Ok(())
            }
            None => Err(RepositoryError::NotFound),
        }
    }

    async fn delete(&self, household_id: HouseholdId, id: RecipeId) -> Result<(), RepositoryError> {
        let mut recipes = self.recipes.lock().unwrap();
        let before = recipes.len();
        recipes.retain(|r| !(r.id == id && r.household_id == household_id));
        if recipes.len() == before {
            Err(RepositoryError::NotFound)
        } else {
            Ok(())
        }
    }
}
