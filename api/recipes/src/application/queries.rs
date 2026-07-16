//! Use cases de lecture des recettes : get (détail), list (grille), search
//! (recherche par titre). Toutes scopées à un [`HouseholdId`].

use kernel::{HouseholdId, RecipeId};

use crate::domain::{Recipe, RecipeRepository};

// --- Get ------------------------------------------------------------------

/// Query : détail d'une recette du foyer.
#[derive(Debug, Clone)]
pub struct GetRecipeQuery {
    /// Foyer propriétaire (scope).
    pub household_id: HouseholdId,
    /// Recette demandée.
    pub recipe_id: RecipeId,
}

/// Résultat d'un get.
#[derive(Debug)]
pub enum GetRecipeResponse {
    /// Recette trouvée.
    Found(Recipe),
    /// Recette absente du foyer.
    NotFound,
    /// Panne technique.
    Unavailable,
}

/// Handler du get.
pub struct GetRecipeHandler<'a> {
    recipes: &'a dyn RecipeRepository,
}

impl<'a> GetRecipeHandler<'a> {
    /// Construit le handler.
    #[must_use]
    pub fn new(recipes: &'a dyn RecipeRepository) -> Self {
        Self { recipes }
    }

    /// Exécute la lecture. Ne renvoie jamais d'erreur.
    pub async fn handle(&self, query: GetRecipeQuery) -> GetRecipeResponse {
        match self.recipes.find(query.household_id, query.recipe_id).await {
            Ok(Some(recipe)) => GetRecipeResponse::Found(recipe),
            Ok(None) => GetRecipeResponse::NotFound,
            Err(_) => GetRecipeResponse::Unavailable,
        }
    }
}

// --- List / Search --------------------------------------------------------

/// Query : liste (ou recherche) des recettes du foyer. `query` vide ⇒ liste
/// complète.
#[derive(Debug, Clone)]
pub struct ListRecipesQuery {
    /// Foyer propriétaire (scope).
    pub household_id: HouseholdId,
    /// Filtre de recherche par titre (insensible à la casse). Vide = tout.
    pub search: Option<String>,
}

/// Résultat d'une liste / recherche.
#[derive(Debug)]
pub enum ListRecipesResponse {
    /// Recettes correspondantes (ordre défini par le repo).
    Listed(Vec<Recipe>),
    /// Panne technique.
    Unavailable,
}

/// Handler de la liste / recherche.
pub struct ListRecipesHandler<'a> {
    recipes: &'a dyn RecipeRepository,
}

impl<'a> ListRecipesHandler<'a> {
    /// Construit le handler.
    #[must_use]
    pub fn new(recipes: &'a dyn RecipeRepository) -> Self {
        Self { recipes }
    }

    /// Exécute la liste ou la recherche selon la présence d'un filtre.
    pub async fn handle(&self, query: ListRecipesQuery) -> ListRecipesResponse {
        let result = match query.search.as_deref().map(str::trim) {
            Some(needle) if !needle.is_empty() => {
                self.recipes.search(query.household_id, needle).await
            }
            _ => self.recipes.list(query.household_id).await,
        };
        match result {
            Ok(recipes) => ListRecipesResponse::Listed(recipes),
            Err(_) => ListRecipesResponse::Unavailable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{sample_recipe, InMemoryRecipes};

    #[tokio::test]
    async fn get_returns_a_stored_recipe() {
        let household = HouseholdId::new();
        let recipe = sample_recipe(household, "Ratatouille");
        let id = recipe.id;
        let repo = InMemoryRecipes::with(vec![recipe]);

        let response = GetRecipeHandler::new(&repo)
            .handle(GetRecipeQuery {
                household_id: household,
                recipe_id: id,
            })
            .await;
        assert!(matches!(response, GetRecipeResponse::Found(r) if r.id == id));
    }

    #[tokio::test]
    async fn get_scopes_to_household() {
        let owner = HouseholdId::new();
        let other = HouseholdId::new();
        let recipe = sample_recipe(owner, "Ratatouille");
        let id = recipe.id;
        let repo = InMemoryRecipes::with(vec![recipe]);

        let response = GetRecipeHandler::new(&repo)
            .handle(GetRecipeQuery {
                household_id: other,
                recipe_id: id,
            })
            .await;
        assert!(matches!(response, GetRecipeResponse::NotFound));
    }

    #[tokio::test]
    async fn empty_search_lists_everything() {
        let household = HouseholdId::new();
        let repo = InMemoryRecipes::with(vec![
            sample_recipe(household, "Ratatouille"),
            sample_recipe(household, "Tarte aux pommes"),
        ]);
        let response = ListRecipesHandler::new(&repo)
            .handle(ListRecipesQuery {
                household_id: household,
                search: Some("  ".to_owned()),
            })
            .await;
        match response {
            ListRecipesResponse::Listed(recipes) => assert_eq!(recipes.len(), 2),
            other => panic!("attendu Listed, obtenu {other:?}"),
        }
    }

    #[tokio::test]
    async fn list_is_sorted_by_title() {
        let household = HouseholdId::new();
        let repo = InMemoryRecipes::with(vec![
            sample_recipe(household, "Tarte aux pommes"),
            sample_recipe(household, "ratatouille"),
        ]);
        let response = ListRecipesHandler::new(&repo)
            .handle(ListRecipesQuery {
                household_id: household,
                search: None,
            })
            .await;
        match response {
            ListRecipesResponse::Listed(recipes) => {
                let titles: Vec<&str> = recipes.iter().map(|r| r.title.as_str()).collect();
                assert_eq!(titles, ["ratatouille", "Tarte aux pommes"]);
            }
            other => panic!("attendu Listed, obtenu {other:?}"),
        }
    }

    #[tokio::test]
    async fn search_filters_by_title_case_insensitive() {
        let household = HouseholdId::new();
        let repo = InMemoryRecipes::with(vec![
            sample_recipe(household, "Ratatouille"),
            sample_recipe(household, "Tarte aux pommes"),
        ]);
        let response = ListRecipesHandler::new(&repo)
            .handle(ListRecipesQuery {
                household_id: household,
                search: Some("TARTE".to_owned()),
            })
            .await;
        match response {
            ListRecipesResponse::Listed(recipes) => {
                assert_eq!(recipes.len(), 1);
                assert_eq!(recipes[0].title, "Tarte aux pommes");
            }
            other => panic!("attendu Listed, obtenu {other:?}"),
        }
    }
}
