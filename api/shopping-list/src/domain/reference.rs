//! Référentiel des ingrédients : poids moyens par pièce, flag `countable` et
//! catégorie. Seedé depuis `data/ingredients.yaml` (cf. `plan.md`).
//!
//! Le service de [conversion](super::conversion) interroge un
//! [`ReferenceCatalog`] pour transformer un grammage agrégé en nombre de
//! pièces achetables. Un ingrédient **absent** du catalogue est un vrac
//! (farine, lait…) : il reste affiché dans son unité d'origine.

use std::collections::HashMap;

/// Poids moyen et métadonnées d'un ingrédient « à la pièce ».
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngredientReference {
    /// Nom canonique de l'ingrédient (ex. `"courgette"`).
    pub name: String,
    /// Catégorie de rayon, pour le regroupement en liste (ex. `"legumes"`).
    pub category: String,
    /// Poids moyen d'une pièce, en grammes (ex. `250` pour une courgette).
    pub avg_weight_g: u32,
    /// Si `true`, l'ingrédient s'achète et s'affiche **toujours** en pièces
    /// (œufs, gousses d'ail…) et n'est jamais reconverti en grammes.
    pub countable: bool,
}

impl IngredientReference {
    /// Construit une référence.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        category: impl Into<String>,
        avg_weight_g: u32,
        countable: bool,
    ) -> Self {
        Self {
            name: name.into(),
            category: category.into(),
            avg_weight_g,
            countable,
        }
    }
}

/// Normalise un nom d'ingrédient pour la comparaison / l'indexation :
/// suppression des espaces de bord et passage en minuscules.
///
/// Garantit que `"Courgette"`, `" courgette "` et `"courgette"` désignent la
/// même entrée. Le rapprochement des pluriels/synonymes est hors périmètre du
/// domaine (il relève de l'import/seed).
#[must_use]
pub fn normalize_name(name: &str) -> String {
    name.trim().to_lowercase()
}

/// Référentiel indexé par nom normalisé, interrogé par le service de
/// conversion.
#[derive(Debug, Clone, Default)]
pub struct ReferenceCatalog {
    by_name: HashMap<String, IngredientReference>,
}

impl ReferenceCatalog {
    /// Catalogue vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insère une référence. En cas de nom déjà présent (après
    /// normalisation), la dernière insertion l'emporte.
    pub fn insert(&mut self, reference: IngredientReference) {
        let key = normalize_name(&reference.name);
        self.by_name.insert(key, reference);
    }

    /// Recherche une référence par nom (insensible à la casse et aux espaces
    /// de bord).
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&IngredientReference> {
        self.by_name.get(&normalize_name(name))
    }

    /// Nombre de références.
    #[must_use]
    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    /// `true` si le catalogue est vide.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }
}

impl FromIterator<IngredientReference> for ReferenceCatalog {
    fn from_iter<T: IntoIterator<Item = IngredientReference>>(iter: T) -> Self {
        let mut catalog = ReferenceCatalog::new();
        for reference in iter {
            catalog.insert(reference);
        }
        catalog
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_trims_and_lowercases() {
        assert_eq!(normalize_name("  Courgette "), "courgette");
        assert_eq!(normalize_name("Champignon de Paris"), "champignon de paris");
    }

    #[test]
    fn find_is_case_and_whitespace_insensitive() {
        let catalog = ReferenceCatalog::from_iter([IngredientReference::new(
            "courgette",
            "legumes",
            250,
            false,
        )]);

        assert_eq!(catalog.find("courgette").unwrap().avg_weight_g, 250);
        assert_eq!(catalog.find("  COURGETTE ").unwrap().avg_weight_g, 250);
        assert!(catalog.find("aubergine").is_none());
    }

    #[test]
    fn insert_last_write_wins_on_normalized_name() {
        let mut catalog = ReferenceCatalog::new();
        catalog.insert(IngredientReference::new("Oeuf", "cremerie", 50, true));
        catalog.insert(IngredientReference::new("oeuf", "cremerie", 55, true));

        assert_eq!(catalog.len(), 1);
        assert_eq!(catalog.find("oeuf").unwrap().avg_weight_g, 55);
    }

    #[test]
    fn empty_catalog() {
        let catalog = ReferenceCatalog::new();
        assert!(catalog.is_empty());
        assert_eq!(catalog.len(), 0);
    }
}
