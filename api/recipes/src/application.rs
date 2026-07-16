//! Couche application de `recipes` : use cases.
//!
//! Chaque use case suit le pattern Command/Query + Handler + Response ; un
//! Handler retourne toujours un objet Response, jamais une exception qui
//! remonterait à la présentation.

use kernel::{Quantity, Unit};

use crate::domain::RecipeIngredient;

/// Écritures : Create / Update / Delete.
pub mod commands;
/// Lectures : Get / List / Search.
pub mod queries;

/// Ingrédient tel que reçu de la présentation (types bruts). Converti en
/// [`RecipeIngredient`] du domaine, avec validation.
#[derive(Debug, Clone)]
pub struct IngredientInput {
    /// Nom de l'ingrédient.
    pub name: String,
    /// Montant (doit être strictement positif).
    pub amount: f64,
    /// Unité.
    pub unit: Unit,
}

/// Convertit des ingrédients bruts en ingrédients de domaine validés.
///
/// # Errors
/// Renvoie un message d'erreur lisible si une quantité est invalide (montant
/// non positif) ou un nom vide.
pub(crate) fn build_ingredients(
    inputs: Vec<IngredientInput>,
) -> Result<Vec<RecipeIngredient>, String> {
    inputs
        .into_iter()
        .map(|input| {
            let quantity = Quantity::new(input.amount, input.unit).map_err(|e| e.to_string())?;
            RecipeIngredient::new(input.name, quantity).map_err(|e| e.to_string())
        })
        .collect()
}
