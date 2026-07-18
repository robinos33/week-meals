//! Représentation YAML du référentiel d'ingrédients (`data/ingredients.yaml`).
//!
//! Contrat du fichier versionné : une liste d'entrées `name` / `category` /
//! `avg_weight_g`, et un `countable` optionnel (ingrédients qui s'achètent
//! toujours en pièces, comme les œufs).

use serde::{Deserialize, Serialize};
use shopping_list::domain::IngredientReference;

/// Le fichier de référentiel dans son ensemble.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceFile {
    /// Version du format (informative).
    #[serde(default)]
    pub version: u32,
    /// Entrées du référentiel.
    pub ingredients: Vec<IngredientYaml>,
}

/// Une entrée du référentiel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngredientYaml {
    /// Nom canonique (ex. `courgette`).
    pub name: String,
    /// Rayon (ex. `legumes`).
    pub category: String,
    /// Poids moyen d'une pièce, en grammes.
    pub avg_weight_g: u32,
    /// S'achète toujours en pièces (jamais reconverti en grammes).
    #[serde(default)]
    pub countable: bool,
}

impl From<IngredientYaml> for IngredientReference {
    fn from(entry: IngredientYaml) -> Self {
        IngredientReference::new(
            entry.name,
            entry.category,
            entry.avg_weight_g,
            entry.countable,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
version: 1
ingredients:
  - name: courgette
    category: legumes
    avg_weight_g: 250
  - name: oeuf
    category: cremerie
    avg_weight_g: 55
    countable: true
"#;

    #[test]
    fn parses_the_reference_format() {
        let file: ReferenceFile = serde_yaml::from_str(SAMPLE).unwrap();
        assert_eq!(file.version, 1);
        assert_eq!(file.ingredients.len(), 2);
        assert_eq!(file.ingredients[0].name, "courgette");
        // `countable` est optionnel et vaut false par défaut.
        assert!(!file.ingredients[0].countable);
        assert!(file.ingredients[1].countable);
    }

    #[test]
    fn converts_to_the_domain_reference() {
        let file: ReferenceFile = serde_yaml::from_str(SAMPLE).unwrap();
        let reference = IngredientReference::from(file.ingredients[1].clone());
        assert_eq!(reference.name, "oeuf");
        assert_eq!(reference.avg_weight_g, 55);
        assert!(reference.countable);
    }
}
