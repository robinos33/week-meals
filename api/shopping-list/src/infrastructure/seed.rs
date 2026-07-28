//! Chargement du dictionnaire versionné (`data/ingredients.yaml`).
//!
//! Contrat du fichier : une liste d'entrées `name` / `category`, un
//! `avg_weight_g` **facultatif** (absent = ne s'achète pas à la pièce), un
//! `countable` et une `unit` optionnels, et des `aliases` — les formulations
//! rencontrées dans les recettes, rapprochées du nom canonique.
//!
//! Le fichier est la source de vérité, la base n'en est qu'un cache : le seed
//! est un **upsert par nom**, rejouable sans effet de bord. Il est donc joué à
//! la fois par la CLI (`weekmeals seed-ingredients`) et au démarrage du
//! serveur, pour qu'un déploiement n'oublie jamais le dictionnaire.

use std::path::Path;

use kernel::{RepositoryError, Unit};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::domain::IngredientReference;

use super::{parse_unit, SqlxReferenceRepository};

/// Le fichier de dictionnaire dans son ensemble.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceFile {
    /// Version du format (informative).
    #[serde(default)]
    pub version: u32,
    /// Entrées du dictionnaire.
    pub ingredients: Vec<IngredientYaml>,
}

/// Une entrée du dictionnaire.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngredientYaml {
    /// Nom canonique (ex. `courgette`).
    pub name: String,
    /// Rayon (ex. `legumes`).
    pub category: String,
    /// Poids moyen d'une pièce, en grammes. Absent pour ce qui ne s'achète
    /// pas à la pièce (farine, lait…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avg_weight_g: Option<u32>,
    /// S'achète toujours en pièces (jamais reconverti en grammes).
    #[serde(default)]
    pub countable: bool,
    /// Unité proposée à la saisie manuelle. Défaut : la pièce si un poids
    /// moyen est connu, le gramme sinon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// Synonymes et formulations rencontrées dans les recettes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub aliases: Vec<String>,
}

impl From<IngredientYaml> for IngredientReference {
    fn from(entry: IngredientYaml) -> Self {
        // L'unité de saisie découle du reste quand elle n'est pas donnée : ce
        // qui a un poids moyen s'achète à la pièce, le reste se pèse.
        let unit = entry
            .unit
            .as_deref()
            .and_then(|unit| parse_unit(unit).ok())
            .unwrap_or(if entry.avg_weight_g.is_some() {
                Unit::Piece
            } else {
                Unit::G
            });
        Self {
            name: entry.name,
            category: entry.category,
            avg_weight_g: entry.avg_weight_g,
            countable: entry.countable,
            aliases: entry.aliases,
            unit,
        }
    }
}

/// Ce qui peut rater au chargement du dictionnaire.
#[derive(Debug, thiserror::Error)]
pub enum SeedError {
    /// Fichier illisible ou absent.
    #[error("lecture du dictionnaire : {0}")]
    Read(#[from] std::io::Error),
    /// YAML mal formé ou hors contrat.
    #[error("dictionnaire mal formé : {0}")]
    Parse(#[from] serde_yaml::Error),
    /// Écriture en base impossible.
    #[error("écriture du dictionnaire : {0}")]
    Store(#[from] RepositoryError),
}

/// Analyse le contenu YAML d'un dictionnaire.
///
/// # Errors
/// [`SeedError::Parse`] si le YAML ne respecte pas le contrat.
pub fn parse_references(yaml: &str) -> Result<Vec<IngredientReference>, SeedError> {
    let file: ReferenceFile = serde_yaml::from_str(yaml)?;
    Ok(file
        .ingredients
        .into_iter()
        .map(IngredientReference::from)
        .collect())
}

/// Lit un fichier de dictionnaire.
///
/// # Errors
/// [`SeedError::Read`] si le fichier est illisible, [`SeedError::Parse`] s'il
/// est hors contrat.
pub fn load_references(path: &Path) -> Result<Vec<IngredientReference>, SeedError> {
    parse_references(&std::fs::read_to_string(path)?)
}

/// Charge le dictionnaire depuis `path` et l'écrit en base (upsert par nom).
/// Renvoie le nombre d'entrées écrites.
///
/// # Errors
/// Voir [`SeedError`].
pub async fn seed_from_file(pool: &SqlitePool, path: &Path) -> Result<u64, SeedError> {
    let references = load_references(path)?;
    let written = SqlxReferenceRepository::new(pool.clone())
        .upsert_all(&references)
        .await?;
    Ok(written)
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
    aliases: [courgettes vertes]
  - name: oeuf
    category: cremerie
    avg_weight_g: 55
    countable: true
  - name: lait
    category: cremerie
    unit: ml
"#;

    #[test]
    fn parses_the_dictionary_format() {
        let references = parse_references(SAMPLE).unwrap();
        assert_eq!(references.len(), 3);
        assert_eq!(references[0].name, "courgette");
        assert_eq!(references[0].aliases, ["courgettes vertes"]);
        // `countable` est optionnel et vaut false par défaut.
        assert!(!references[0].countable);
        assert!(references[1].countable);
    }

    #[test]
    fn derives_the_input_unit_from_the_entry() {
        let references = parse_references(SAMPLE).unwrap();
        assert_eq!(references[0].unit, Unit::Piece, "poids moyen connu");
        assert_eq!(references[2].unit, Unit::Ml, "unité déclarée");
    }

    #[test]
    fn an_entry_without_average_weight_is_not_bought_by_the_piece() {
        let references = parse_references(SAMPLE).unwrap();
        assert_eq!(references[2].name, "lait");
        assert_eq!(references[2].avg_weight_g, None);
    }

    #[test]
    fn rejects_a_malformed_file() {
        assert!(matches!(
            parse_references("ingredients: 12"),
            Err(SeedError::Parse(_))
        ));
    }
}
