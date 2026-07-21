//! Use cases d'écriture des recettes : création, mise à jour, suppression.
//! Toutes les opérations sont scopées à un [`HouseholdId`].

use kernel::{HouseholdId, RecipeId, RepositoryError};

use super::{build_ingredients, IngredientInput};
use crate::domain::{Recipe, RecipeRepository};

/// Champs communs à la création et à la mise à jour d'une recette.
#[derive(Debug, Clone)]
pub struct RecipeFields {
    /// Titre (obligatoire, non vide).
    pub title: String,
    /// Temps de préparation en minutes.
    pub prep_time_min: Option<u32>,
    /// Temps de cuisson en minutes.
    pub cook_time_min: Option<u32>,
    /// Photo (URL R2 ou chemin de seed).
    pub photo: Option<String>,
    /// Ingrédients.
    pub ingredients: Vec<IngredientInput>,
    /// Étapes de préparation ordonnées.
    pub steps: Vec<String>,
}

// --- Create ---------------------------------------------------------------

/// Command de création d'une recette dans un foyer.
#[derive(Debug, Clone)]
pub struct CreateRecipeCommand {
    /// Foyer propriétaire.
    pub household_id: HouseholdId,
    /// Champs de la recette.
    pub fields: RecipeFields,
}

/// Résultat d'une création.
#[derive(Debug)]
pub enum CreateRecipeResponse {
    /// Recette créée.
    Created(Recipe),
    /// Entrée invalide (titre vide, quantité non positive…).
    Invalid(String),
    /// Panne technique.
    Unavailable,
}

/// Handler de création.
pub struct CreateRecipeHandler<'a> {
    recipes: &'a dyn RecipeRepository,
}

impl<'a> CreateRecipeHandler<'a> {
    /// Construit le handler.
    #[must_use]
    pub fn new(recipes: &'a dyn RecipeRepository) -> Self {
        Self { recipes }
    }

    /// Exécute la création. Ne renvoie jamais d'erreur.
    pub async fn handle(&self, command: CreateRecipeCommand) -> CreateRecipeResponse {
        let ingredients = match build_ingredients(command.fields.ingredients) {
            Ok(ingredients) => ingredients,
            Err(message) => return CreateRecipeResponse::Invalid(message),
        };
        let recipe = match Recipe::new(
            command.household_id,
            command.fields.title,
            command.fields.prep_time_min,
            command.fields.cook_time_min,
            command.fields.photo,
            ingredients,
            command.fields.steps,
        ) {
            Ok(recipe) => recipe,
            Err(error) => return CreateRecipeResponse::Invalid(error.to_string()),
        };

        match self.recipes.create(&recipe).await {
            Ok(()) => CreateRecipeResponse::Created(recipe),
            Err(_) => CreateRecipeResponse::Unavailable,
        }
    }
}

// --- Update ---------------------------------------------------------------

/// Command de mise à jour d'une recette existante.
#[derive(Debug, Clone)]
pub struct UpdateRecipeCommand {
    /// Foyer propriétaire (scope).
    pub household_id: HouseholdId,
    /// Recette à modifier.
    pub recipe_id: RecipeId,
    /// Nouveaux champs (remplacement complet).
    pub fields: RecipeFields,
}

/// Résultat d'une mise à jour.
#[derive(Debug)]
pub enum UpdateRecipeResponse {
    /// Recette mise à jour.
    Updated(Recipe),
    /// Recette absente du foyer.
    NotFound,
    /// Entrée invalide.
    Invalid(String),
    /// Panne technique.
    Unavailable,
}

/// Handler de mise à jour.
pub struct UpdateRecipeHandler<'a> {
    recipes: &'a dyn RecipeRepository,
}

impl<'a> UpdateRecipeHandler<'a> {
    /// Construit le handler.
    #[must_use]
    pub fn new(recipes: &'a dyn RecipeRepository) -> Self {
        Self { recipes }
    }

    /// Exécute la mise à jour. Ne renvoie jamais d'erreur.
    pub async fn handle(&self, command: UpdateRecipeCommand) -> UpdateRecipeResponse {
        let ingredients = match build_ingredients(command.fields.ingredients) {
            Ok(ingredients) => ingredients,
            Err(message) => return UpdateRecipeResponse::Invalid(message),
        };
        let recipe = match Recipe::from_parts(
            command.recipe_id,
            command.household_id,
            command.fields.title,
            command.fields.prep_time_min,
            command.fields.cook_time_min,
            command.fields.photo,
            ingredients,
            command.fields.steps,
        ) {
            Ok(recipe) => recipe,
            Err(error) => return UpdateRecipeResponse::Invalid(error.to_string()),
        };

        match self.recipes.update(&recipe).await {
            // La mise à jour ne touche pas `cooked_count` (compteur du podium,
            // #58) ; on relit la recette pour renvoyer sa valeur à jour plutôt
            // que le `0` porté par l'entité fraîchement construite.
            Ok(()) => match self
                .recipes
                .find(command.household_id, command.recipe_id)
                .await
            {
                Ok(Some(fresh)) => UpdateRecipeResponse::Updated(fresh),
                _ => UpdateRecipeResponse::Updated(recipe),
            },
            Err(RepositoryError::NotFound) => UpdateRecipeResponse::NotFound,
            Err(_) => UpdateRecipeResponse::Unavailable,
        }
    }
}

// --- Delete ---------------------------------------------------------------

/// Command de suppression d'une recette.
#[derive(Debug, Clone)]
pub struct DeleteRecipeCommand {
    /// Foyer propriétaire (scope).
    pub household_id: HouseholdId,
    /// Recette à supprimer.
    pub recipe_id: RecipeId,
}

/// Résultat d'une suppression.
#[derive(Debug, PartialEq, Eq)]
pub enum DeleteRecipeResponse {
    /// Recette supprimée.
    Deleted,
    /// Recette absente du foyer.
    NotFound,
    /// Panne technique.
    Unavailable,
}

/// Handler de suppression.
pub struct DeleteRecipeHandler<'a> {
    recipes: &'a dyn RecipeRepository,
}

impl<'a> DeleteRecipeHandler<'a> {
    /// Construit le handler.
    #[must_use]
    pub fn new(recipes: &'a dyn RecipeRepository) -> Self {
        Self { recipes }
    }

    /// Exécute la suppression. Ne renvoie jamais d'erreur.
    pub async fn handle(&self, command: DeleteRecipeCommand) -> DeleteRecipeResponse {
        match self
            .recipes
            .delete(command.household_id, command.recipe_id)
            .await
        {
            Ok(()) => DeleteRecipeResponse::Deleted,
            Err(RepositoryError::NotFound) => DeleteRecipeResponse::NotFound,
            Err(_) => DeleteRecipeResponse::Unavailable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::IngredientInput;
    use crate::testing::InMemoryRecipes;
    use kernel::Unit;

    fn fields(title: &str) -> RecipeFields {
        RecipeFields {
            title: title.to_owned(),
            prep_time_min: Some(10),
            cook_time_min: Some(20),
            photo: None,
            ingredients: vec![IngredientInput {
                name: "courgette".to_owned(),
                amount: 600.0,
                unit: Unit::G,
            }],
            steps: vec!["Émincer.".to_owned()],
        }
    }

    #[tokio::test]
    async fn create_persists_a_valid_recipe() {
        let repo = InMemoryRecipes::default();
        let household = HouseholdId::new();
        let response = CreateRecipeHandler::new(&repo)
            .handle(CreateRecipeCommand {
                household_id: household,
                fields: fields("Ratatouille"),
            })
            .await;

        let recipe = match response {
            CreateRecipeResponse::Created(recipe) => recipe,
            other => panic!("attendu Created, obtenu {other:?}"),
        };
        assert_eq!(recipe.title, "Ratatouille");
        assert_eq!(repo.count(), 1);
    }

    #[tokio::test]
    async fn create_rejects_blank_title() {
        let repo = InMemoryRecipes::default();
        let response = CreateRecipeHandler::new(&repo)
            .handle(CreateRecipeCommand {
                household_id: HouseholdId::new(),
                fields: fields("   "),
            })
            .await;
        assert!(matches!(response, CreateRecipeResponse::Invalid(_)));
        assert_eq!(repo.count(), 0);
    }

    #[tokio::test]
    async fn create_rejects_non_positive_quantity() {
        let repo = InMemoryRecipes::default();
        let mut fields = fields("Soupe");
        fields.ingredients[0].amount = 0.0;
        let response = CreateRecipeHandler::new(&repo)
            .handle(CreateRecipeCommand {
                household_id: HouseholdId::new(),
                fields,
            })
            .await;
        assert!(matches!(response, CreateRecipeResponse::Invalid(_)));
    }

    #[tokio::test]
    async fn update_missing_recipe_is_not_found() {
        let repo = InMemoryRecipes::default();
        let response = UpdateRecipeHandler::new(&repo)
            .handle(UpdateRecipeCommand {
                household_id: HouseholdId::new(),
                recipe_id: RecipeId::new(),
                fields: fields("Ratatouille"),
            })
            .await;
        assert!(matches!(response, UpdateRecipeResponse::NotFound));
    }

    #[tokio::test]
    async fn delete_missing_recipe_is_not_found() {
        let repo = InMemoryRecipes::default();
        let response = DeleteRecipeHandler::new(&repo)
            .handle(DeleteRecipeCommand {
                household_id: HouseholdId::new(),
                recipe_id: RecipeId::new(),
            })
            .await;
        assert_eq!(response, DeleteRecipeResponse::NotFound);
    }
}
