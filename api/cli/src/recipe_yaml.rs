//! Représentation YAML d'une recette et conversions vers/depuis le domaine.
//!
//! Le format est le contrat public des seeds (`data/recipes/*.yaml`, cf.
//! ADR-0003) : `title`, `prep_time_min`/`cook_time_min` optionnels, `photo`
//! optionnelle, `ingredients` (`name`/`quantity`/`unit`) et `steps` ordonnées.

use anyhow::{Context, Result};
use kernel::{HouseholdId, Quantity, RecipeId, Unit};
use recipes::domain::{Recipe, RecipeIngredient};
use serde::{Deserialize, Serialize};

/// Une recette telle qu'écrite dans un fichier YAML de seed / export.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecipeYaml {
    /// Titre.
    pub title: String,
    /// Temps de préparation en minutes (optionnel).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prep_time_min: Option<u32>,
    /// Temps de cuisson en minutes (optionnel).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cook_time_min: Option<u32>,
    /// Chemin relatif au dossier de seed ou URL (optionnel).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub photo: Option<String>,
    /// Ingrédients.
    #[serde(default)]
    pub ingredients: Vec<IngredientYaml>,
    /// Étapes de préparation, ordonnées.
    #[serde(default)]
    pub steps: Vec<String>,
}

/// Un ingrédient dans un fichier YAML.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IngredientYaml {
    /// Nom libre (ex. « courgette »).
    pub name: String,
    /// Montant, dans l'unité `unit`.
    pub quantity: f64,
    /// Unité (`g`, `kg`, `ml`, `l`, `piece`).
    pub unit: Unit,
}

impl RecipeYaml {
    /// Reconstruit une recette du domaine avec l'identifiant `id` fourni
    /// (nouveau à la création, existant lors d'un upsert) et le foyer `household`.
    ///
    /// # Errors
    /// Remonte une erreur si une quantité est invalide (≤ 0, cf. `Quantity`),
    /// un nom d'ingrédient vide, ou un invariant de recette violé (titre vide,
    /// temps hors bornes).
    pub fn into_recipe(self, id: RecipeId, household: HouseholdId) -> Result<Recipe> {
        let title = self.title;
        let ingredients = self
            .ingredients
            .into_iter()
            .map(|ingredient| {
                let quantity =
                    Quantity::new(ingredient.quantity, ingredient.unit).with_context(|| {
                        format!(
                            "quantité invalide pour « {} » (recette « {title} »)",
                            ingredient.name
                        )
                    })?;
                RecipeIngredient::new(ingredient.name, quantity)
                    .with_context(|| format!("ingrédient invalide (recette « {title} »)"))
            })
            .collect::<Result<Vec<_>>>()?;

        Recipe::from_parts(
            id,
            household,
            &title,
            self.prep_time_min,
            self.cook_time_min,
            self.photo,
            ingredients,
            self.steps,
        )
        .with_context(|| format!("recette « {title} » invalide"))
    }

    /// Construit un YAML de seed depuis un brouillon extrait d'une page web
    /// (`weekmeals scrape`, #61). Le résultat reste à relire avant import.
    #[must_use]
    pub fn from_scraped(recipe: recipes::domain::ScrapedRecipe) -> Self {
        Self {
            title: recipe.title,
            prep_time_min: recipe.prep_time_min,
            cook_time_min: recipe.cook_time_min,
            photo: recipe.photo,
            ingredients: recipe
                .ingredients
                .into_iter()
                .map(|i| IngredientYaml {
                    name: i.name,
                    quantity: i.amount,
                    unit: i.unit,
                })
                .collect(),
            steps: recipe.steps,
        }
    }

    /// Projette une recette du domaine vers sa forme YAML (pour l'export).
    #[must_use]
    pub fn from_recipe(recipe: &Recipe) -> Self {
        Self {
            title: recipe.title.clone(),
            prep_time_min: recipe.prep_time_min,
            cook_time_min: recipe.cook_time_min,
            photo: recipe.photo.clone(),
            ingredients: recipe
                .ingredients
                .iter()
                .map(|i| IngredientYaml {
                    name: i.name.clone(),
                    quantity: i.quantity.amount(),
                    unit: i.quantity.unit(),
                })
                .collect(),
            steps: recipe.steps.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
title: Ratatouille
prep_time_min: 25
cook_time_min: 45
photo: ~
ingredients:
  - name: courgette
    quantity: 600
    unit: g
  - name: gousse d'ail
    quantity: 3
    unit: piece
steps:
  - Émincer l'ail.
  - Laisser mijoter.
"#;

    #[test]
    fn parses_the_seed_format() {
        let parsed: RecipeYaml = serde_yaml::from_str(SAMPLE).unwrap();
        assert_eq!(parsed.title, "Ratatouille");
        assert_eq!(parsed.prep_time_min, Some(25));
        assert_eq!(parsed.photo, None);
        assert_eq!(parsed.ingredients.len(), 2);
        assert_eq!(parsed.ingredients[1].unit, Unit::Piece);
        assert_eq!(parsed.steps.len(), 2);
    }

    #[test]
    fn converts_to_a_domain_recipe_with_the_given_id() {
        let household = HouseholdId::new();
        let id = RecipeId::new();
        let recipe = serde_yaml::from_str::<RecipeYaml>(SAMPLE)
            .unwrap()
            .into_recipe(id, household)
            .unwrap();
        assert_eq!(recipe.id, id);
        assert_eq!(recipe.household_id, household);
        assert_eq!(recipe.ingredients.len(), 2);
    }

    #[test]
    fn round_trips_through_the_domain() {
        let household = HouseholdId::new();
        let original = serde_yaml::from_str::<RecipeYaml>(SAMPLE).unwrap();
        let recipe = original
            .clone()
            .into_recipe(RecipeId::new(), household)
            .unwrap();
        let back = RecipeYaml::from_recipe(&recipe);
        assert_eq!(back, original);
    }

    #[test]
    fn rejects_a_non_positive_quantity() {
        let yaml = "title: X\ningredients:\n  - name: sel\n    quantity: 0\n    unit: g\n";
        let err = serde_yaml::from_str::<RecipeYaml>(yaml)
            .unwrap()
            .into_recipe(RecipeId::new(), HouseholdId::new())
            .unwrap_err();
        assert!(err.to_string().contains("quantité invalide"));
    }
}
