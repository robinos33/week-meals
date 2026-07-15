//! Couche domaine de `recipes` : entités, value objects, traits de repository
//! et services purs. Aucune dépendance à SQLx/Axum (règle de convention).
//!
//! Modélise une recette (cf. [issue #9], [plan.md — Modèle métier]) :
//! titre, temps de préparation/cuisson, photo optionnelle, ingrédients
//! (`name` + [`Quantity`]) et étapes de préparation ordonnées.
//!
//! [issue #9]: https://github.com/robinos33/week-meals/issues/9
//! [plan.md — Modèle métier]: ../../../docs/plan.md

use kernel::{HouseholdId, Quantity, RecipeId, RepositoryError};

/// Un ingrédient d'une recette : un nom libre et une [`Quantity`].
///
/// Le nom n'est pas contraint par le référentiel `IngredientReference` : un
/// ingrédient sans poids moyen connu reste valide, il ne sera simplement pas
/// reconverti en pièces lors de la génération de liste de courses.
#[derive(Debug, Clone, PartialEq)]
pub struct RecipeIngredient {
    /// Nom de l'ingrédient (ex. « courgette »).
    pub name: String,
    /// Quantité (montant + unité).
    pub quantity: Quantity,
}

impl RecipeIngredient {
    /// Construit un ingrédient. Le nom est nettoyé (trim).
    ///
    /// # Errors
    /// Renvoie [`RecipeError::EmptyIngredientName`] si le nom est vide.
    pub fn new(name: impl Into<String>, quantity: Quantity) -> Result<Self, RecipeError> {
        let name = name.into().trim().to_owned();
        if name.is_empty() {
            return Err(RecipeError::EmptyIngredientName);
        }
        Ok(Self { name, quantity })
    }
}

/// Une recette : agrégat racine du domaine `recipes`, scopé à un foyer.
#[derive(Debug, Clone, PartialEq)]
pub struct Recipe {
    /// Identifiant de la recette.
    pub id: RecipeId,
    /// Foyer propriétaire (toutes les données sont scopées au foyer).
    pub household_id: HouseholdId,
    /// Titre.
    pub title: String,
    /// Photo : URL R2 ou chemin de seed. Optionnelle.
    pub photo: Option<String>,
    /// Temps de préparation en minutes. Optionnel.
    pub prep_time_min: Option<u32>,
    /// Temps de cuisson en minutes. Optionnel.
    pub cook_time_min: Option<u32>,
    /// Ingrédients.
    pub ingredients: Vec<RecipeIngredient>,
    /// Étapes de préparation, dans l'ordre.
    pub steps: Vec<String>,
}

impl Recipe {
    /// Construit une recette en validant ses invariants (titre non vide).
    ///
    /// Génère un nouvel identifiant. Le titre est nettoyé (trim).
    ///
    /// # Errors
    /// Renvoie [`RecipeError::EmptyTitle`] si le titre est vide.
    pub fn new(
        household_id: HouseholdId,
        title: impl Into<String>,
        prep_time_min: Option<u32>,
        cook_time_min: Option<u32>,
        photo: Option<String>,
        ingredients: Vec<RecipeIngredient>,
        steps: Vec<String>,
    ) -> Result<Self, RecipeError> {
        let title = title.into().trim().to_owned();
        if title.is_empty() {
            return Err(RecipeError::EmptyTitle);
        }
        Ok(Self {
            id: RecipeId::new(),
            household_id,
            title,
            photo,
            prep_time_min,
            cook_time_min,
            ingredients,
            steps,
        })
    }

    /// Reconstitue une recette dont l'identifiant est connu — pour une mise à
    /// jour (l'`id` est fourni par l'appelant) ou une lecture depuis la
    /// persistance. Valide et nettoie le titre comme [`Recipe::new`].
    ///
    /// # Errors
    /// Renvoie [`RecipeError::EmptyTitle`] si le titre est vide.
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: RecipeId,
        household_id: HouseholdId,
        title: impl Into<String>,
        prep_time_min: Option<u32>,
        cook_time_min: Option<u32>,
        photo: Option<String>,
        ingredients: Vec<RecipeIngredient>,
        steps: Vec<String>,
    ) -> Result<Self, RecipeError> {
        let title = title.into().trim().to_owned();
        if title.is_empty() {
            return Err(RecipeError::EmptyTitle);
        }
        Ok(Self {
            id,
            household_id,
            title,
            photo,
            prep_time_min,
            cook_time_min,
            ingredients,
            steps,
        })
    }
}

/// Violation d'un invariant du domaine `recipes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RecipeError {
    /// Le titre d'une recette ne peut pas être vide.
    #[error("recipe title must not be empty")]
    EmptyTitle,
    /// Le nom d'un ingrédient ne peut pas être vide.
    #[error("ingredient name must not be empty")]
    EmptyIngredientName,
}

/// Port de persistance des recettes. Implémenté dans la couche infrastructure
/// (SQLx) ; le domaine ne connaît que ce trait.
///
/// Toutes les lectures/écritures sont scopées à un [`HouseholdId`] : une
/// recette n'est jamais accessible hors de son foyer.
#[async_trait::async_trait]
pub trait RecipeRepository: Send + Sync {
    /// Persiste une nouvelle recette.
    async fn create(&self, recipe: &Recipe) -> Result<(), RepositoryError>;

    /// Récupère une recette du foyer par son identifiant, si elle existe.
    async fn find(
        &self,
        household_id: HouseholdId,
        id: RecipeId,
    ) -> Result<Option<Recipe>, RepositoryError>;

    /// Liste les recettes du foyer.
    async fn list(&self, household_id: HouseholdId) -> Result<Vec<Recipe>, RepositoryError>;

    /// Recherche les recettes du foyer dont le titre contient `query`
    /// (insensible à la casse). Une requête vide équivaut à [`Self::list`].
    async fn search(
        &self,
        household_id: HouseholdId,
        query: &str,
    ) -> Result<Vec<Recipe>, RepositoryError>;

    /// Met à jour une recette existante.
    ///
    /// # Errors
    /// [`RepositoryError::NotFound`] si la recette n'existe pas dans le foyer.
    async fn update(&self, recipe: &Recipe) -> Result<(), RepositoryError>;

    /// Supprime une recette du foyer.
    ///
    /// # Errors
    /// [`RepositoryError::NotFound`] si la recette n'existe pas dans le foyer.
    async fn delete(&self, household_id: HouseholdId, id: RecipeId) -> Result<(), RepositoryError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernel::Unit;

    fn ingredient(name: &str, amount: f64, unit: Unit) -> RecipeIngredient {
        RecipeIngredient::new(name, Quantity::new(amount, unit).unwrap()).unwrap()
    }

    #[test]
    fn builds_a_recipe_with_a_fresh_id() {
        let household = HouseholdId::new();
        let recipe = Recipe::new(
            household,
            "Ratatouille",
            Some(25),
            Some(45),
            None,
            vec![
                ingredient("courgette", 600.0, Unit::G),
                ingredient("gousse d'ail", 3.0, Unit::Piece),
            ],
            vec![
                "Émincer l'oignon.".to_owned(),
                "Laisser mijoter.".to_owned(),
            ],
        )
        .unwrap();

        assert_eq!(recipe.household_id, household);
        assert_eq!(recipe.title, "Ratatouille");
        assert_eq!(recipe.ingredients.len(), 2);
        assert_eq!(recipe.steps.len(), 2);
    }

    #[test]
    fn trims_the_title() {
        let recipe = Recipe::new(
            HouseholdId::new(),
            "  Tarte aux pommes  ",
            None,
            None,
            None,
            vec![],
            vec![],
        )
        .unwrap();
        assert_eq!(recipe.title, "Tarte aux pommes");
    }

    #[test]
    fn rejects_a_blank_title() {
        let err =
            Recipe::new(HouseholdId::new(), "   ", None, None, None, vec![], vec![]).unwrap_err();
        assert_eq!(err, RecipeError::EmptyTitle);
    }

    #[test]
    fn rejects_a_blank_ingredient_name() {
        let err = RecipeIngredient::new("  ", Quantity::new(1.0, Unit::G).unwrap()).unwrap_err();
        assert_eq!(err, RecipeError::EmptyIngredientName);
    }
}
