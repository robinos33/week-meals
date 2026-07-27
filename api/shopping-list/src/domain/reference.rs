//! Dictionnaire des ingrédients : nom canonique, synonymes, rayon, poids moyen
//! par pièce et unité d'achat. Seedé depuis `data/ingredients.yaml`.
//!
//! C'est **le** vocabulaire de référence de l'application. Tout nom venu
//! d'ailleurs — une recette importée, une saisie à la main — lui est rapproché
//! par [`ReferenceCatalog::resolve`], qui applique les trois niveaux de
//! tolérance du module [`matching`](super::matching) : clé canonique, clé
//! produit (qualificatifs retirés), puis proximité orthographique. C'est ce qui
//! évite trois lignes « Courgettes », « courgette jaune » et « courgettes »
//! pour un seul légume.
//!
//! Un ingrédient **absent** du dictionnaire reste parfaitement utilisable :
//! il garde son nom et son unité d'origine, simplement sans rayon ni
//! conversion en pièces.

use std::collections::HashMap;

use kernel::Unit;

use super::matching::{canonical_key, core_key, is_close, similarity};

/// Une entrée du dictionnaire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IngredientReference {
    /// Nom canonique de l'ingrédient (ex. `"courgette"`) : c'est lui qui
    /// s'affiche dans la liste, quelle que soit la formulation d'origine.
    pub name: String,
    /// Catégorie de rayon, pour le regroupement en liste (ex. `"legumes"`).
    pub category: String,
    /// Poids moyen d'une pièce, en grammes (ex. `250` pour une courgette).
    /// `None` pour un ingrédient qui ne s'achète pas à la pièce (farine,
    /// lait…) : il reste alors dans son unité d'origine.
    pub avg_weight_g: Option<u32>,
    /// Si `true`, l'ingrédient s'achète et s'affiche **toujours** en pièces
    /// (œufs, gousses d'ail…) et n'est jamais reconverti en grammes.
    pub countable: bool,
    /// Synonymes et formulations rencontrées dans les recettes (`"oeuf"`,
    /// `"jaune d'oeuf"`), rapprochés du même nom canonique.
    pub aliases: Vec<String>,
    /// Unité proposée à la saisie manuelle (pièce, g, mL…).
    pub unit: Unit,
}

impl IngredientReference {
    /// Construit une référence **à la pièce** : le poids moyen permet de
    /// convertir un grammage agrégé en nombre d'articles à acheter.
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
            avg_weight_g: Some(avg_weight_g),
            countable,
            aliases: Vec::new(),
            unit: Unit::Piece,
        }
    }

    /// Construit une référence **en vrac** : pas de poids moyen, donc pas de
    /// conversion en pièces. L'entrée n'apporte « que » le nom canonique, les
    /// synonymes, le rayon et l'unité d'achat — l'essentiel pour dédoublonner.
    #[must_use]
    pub fn bulk(name: impl Into<String>, category: impl Into<String>, unit: Unit) -> Self {
        Self {
            name: name.into(),
            category: category.into(),
            avg_weight_g: None,
            countable: false,
            aliases: Vec::new(),
            unit,
        }
    }

    /// Ajoute des synonymes à la référence.
    #[must_use]
    pub fn with_aliases<S: Into<String>>(mut self, aliases: impl IntoIterator<Item = S>) -> Self {
        self.aliases = aliases.into_iter().map(Into::into).collect();
        self
    }
}

/// Normalise un nom d'ingrédient pour l'indexation exacte : suppression des
/// espaces de bord et passage en minuscules.
///
/// C'est la clé **stricte** : `"Courgette"`, `" courgette "` et `"courgette"`
/// désignent la même entrée. Les pluriels, accents et synonymes relèvent du
/// rapprochement approximatif ([`ReferenceCatalog::resolve`]).
#[must_use]
pub fn normalize_name(name: &str) -> String {
    name.trim().to_lowercase()
}

/// Dictionnaire indexé, interrogé par le service de conversion et par l'ajout
/// manuel.
///
/// Trois index dérivés doublent l'index exact : la clé canonique (accents,
/// pluriels, mots vides), la clé produit (qualificatifs retirés) et, à défaut,
/// un balayage par proximité orthographique. Ils sont reconstruits à chaque
/// insertion — le dictionnaire tient dans quelques centaines d'entrées et n'est
/// chargé qu'une fois par requête.
#[derive(Debug, Clone, Default)]
pub struct ReferenceCatalog {
    /// Entrées dans leur ordre d'insertion (départage les rapprochements).
    entries: Vec<IngredientReference>,
    /// Nom normalisé → rang de l'entrée.
    by_name: HashMap<String, usize>,
    /// Clé canonique du nom **ou d'un synonyme** → rang de l'entrée.
    by_key: HashMap<String, usize>,
    /// Clé produit (qualificatifs retirés) → rang de l'entrée.
    by_core: HashMap<String, usize>,
}

impl ReferenceCatalog {
    /// Catalogue vide.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Insère une référence. En cas de nom déjà présent (après
    /// normalisation), la dernière insertion l'emporte, à sa place d'origine.
    pub fn insert(&mut self, reference: IngredientReference) {
        match self.by_name.get(&normalize_name(&reference.name)) {
            Some(&rank) => self.entries[rank] = reference,
            None => self.entries.push(reference),
        }
        self.reindex();
    }

    /// Recherche une référence par nom exact (insensible à la casse et aux
    /// espaces de bord), sans aucune tolérance.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&IngredientReference> {
        self.by_name
            .get(&normalize_name(name))
            .map(|&i| &self.entries[i])
    }

    /// Rapproche un nom quelconque du dictionnaire, du plus strict au plus
    /// tolérant :
    ///
    /// 1. nom exact (`"Courgette"`) ;
    /// 2. clé canonique du nom ou d'un synonyme (`"courgettes"`, `"œufs"`) ;
    /// 3. clé produit, qualificatifs retirés (`"courgettes jaunes"`) ;
    /// 4. proximité orthographique (`"échalotte"`).
    ///
    /// L'ordre compte : une entrée exacte doit toujours l'emporter sur un
    /// rapprochement — sans quoi « tomate cerise » finirait en « tomate ».
    #[must_use]
    pub fn resolve(&self, name: &str) -> Option<&IngredientReference> {
        if let Some(reference) = self.find(name) {
            return Some(reference);
        }

        let key = canonical_key(name);
        if key.is_empty() {
            return None;
        }
        if let Some(&rank) = self.by_key.get(&key) {
            return Some(&self.entries[rank]);
        }

        let core = core_key(name);
        if !core.is_empty() && core != key {
            if let Some(&rank) = self.by_key.get(&core).or_else(|| self.by_core.get(&core)) {
                return Some(&self.entries[rank]);
            }
        }

        self.closest(&key)
    }

    /// Clé de regroupement d'un nom : le nom canonique du dictionnaire s'il y
    /// est rapproché, sa clé canonique sinon.
    ///
    /// Deux formulations qui partagent cette clé désignent le même produit :
    /// c'est elle qui décide si l'on cumule ou si l'on ouvre une ligne.
    #[must_use]
    pub fn group_key(&self, name: &str) -> String {
        self.resolve(name)
            .map_or_else(|| canonical_key(name), |found| normalize_name(&found.name))
    }

    /// Entrées du dictionnaire, dans leur ordre d'insertion.
    #[must_use]
    pub fn entries(&self) -> &[IngredientReference] {
        &self.entries
    }

    /// Nombre de références.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` si le catalogue est vide.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Entrée dont le nom (ou un synonyme) est orthographiquement le plus
    /// proche, au-delà du seuil de similarité. Balayage dans l'ordre
    /// d'insertion : à égalité de score, la première entrée gagne — le résultat
    /// ne dépend jamais de l'ordre de parcours d'une table de hachage.
    fn closest(&self, key: &str) -> Option<&IngredientReference> {
        let mut best: Option<(f64, usize)> = None;
        for (rank, entry) in self.entries.iter().enumerate() {
            let candidates = std::iter::once(&entry.name).chain(entry.aliases.iter());
            for candidate in candidates {
                let candidate_key = canonical_key(candidate);
                if !is_close(key, &candidate_key) {
                    continue;
                }
                let score = similarity(key, &candidate_key);
                if best.is_none_or(|(previous, _)| score > previous) {
                    best = Some((score, rank));
                }
            }
        }
        best.map(|(_, rank)| &self.entries[rank])
    }

    /// Reconstruit les index dérivés. Le premier arrivé gagne sur les clés
    /// dérivées (deux entrées peuvent partager une clé produit : « tomate » et
    /// « tomate cerise » ont toutes deux « tomate » en tête).
    fn reindex(&mut self) {
        self.by_name.clear();
        self.by_key.clear();
        self.by_core.clear();

        for (rank, entry) in self.entries.iter().enumerate() {
            self.by_name.insert(normalize_name(&entry.name), rank);
            for name in std::iter::once(&entry.name).chain(entry.aliases.iter()) {
                let key = canonical_key(name);
                if !key.is_empty() {
                    self.by_key.entry(key).or_insert(rank);
                }
                let core = core_key(name);
                if !core.is_empty() {
                    self.by_core.entry(core).or_insert(rank);
                }
            }
        }
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

    fn catalog() -> ReferenceCatalog {
        ReferenceCatalog::from_iter([
            IngredientReference::new("courgette", "legumes", 250, false),
            IngredientReference::new("tomate", "legumes", 120, false),
            IngredientReference::new("tomate cerise", "legumes", 15, false),
            IngredientReference::new("échalote", "legumes", 40, false),
            IngredientReference::new("œuf", "cremerie", 55, true).with_aliases(["oeuf", "œufs"]),
            IngredientReference::bulk("lait", "cremerie", Unit::Ml),
            IngredientReference::bulk("lait de coco", "epicerie", Unit::Ml),
        ])
    }

    #[test]
    fn normalize_trims_and_lowercases() {
        assert_eq!(normalize_name("  Courgette "), "courgette");
        assert_eq!(normalize_name("Champignon de Paris"), "champignon de paris");
    }

    #[test]
    fn find_is_case_and_whitespace_insensitive() {
        let catalog = catalog();
        assert_eq!(catalog.find("courgette").unwrap().avg_weight_g, Some(250));
        assert_eq!(
            catalog.find("  COURGETTE ").unwrap().avg_weight_g,
            Some(250)
        );
        assert!(catalog.find("aubergine").is_none());
    }

    #[test]
    fn find_stays_strict() {
        // `find` sert l'indexation exacte : aucune tolérance, sinon le seed et
        // le rapprochement ne diraient plus la même chose.
        assert!(catalog().find("courgettes").is_none());
    }

    #[test]
    fn insert_last_write_wins_on_normalized_name() {
        let mut catalog = ReferenceCatalog::new();
        catalog.insert(IngredientReference::new("Oeuf", "cremerie", 50, true));
        catalog.insert(IngredientReference::new("oeuf", "cremerie", 55, true));

        assert_eq!(catalog.len(), 1);
        assert_eq!(catalog.find("oeuf").unwrap().avg_weight_g, Some(55));
    }

    #[test]
    fn resolve_matches_plurals_and_accents() {
        let catalog = catalog();
        assert_eq!(catalog.resolve("Courgettes").unwrap().name, "courgette");
        assert_eq!(catalog.resolve("echalotes").unwrap().name, "échalote");
    }

    #[test]
    fn resolve_matches_aliases() {
        let catalog = catalog();
        assert_eq!(catalog.resolve("oeufs").unwrap().name, "œuf");
        assert_eq!(catalog.resolve("Œuf").unwrap().name, "œuf");
    }

    #[test]
    fn resolve_strips_qualifiers() {
        let catalog = catalog();
        assert_eq!(
            catalog.resolve("courgettes jaunes").unwrap().name,
            "courgette"
        );
        assert_eq!(catalog.resolve("œufs bio").unwrap().name, "œuf");
    }

    #[test]
    fn resolve_prefers_the_exact_entry_over_a_broader_one() {
        // « tomate cerise » ne doit pas se faire absorber par « tomate ».
        let catalog = catalog();
        assert_eq!(
            catalog.resolve("tomates cerises").unwrap().name,
            "tomate cerise"
        );
    }

    #[test]
    fn resolve_keeps_distinct_products_apart() {
        // « lait de coco » n'est pas du lait : les mots supplémentaires ne sont
        // pas des qualificatifs, l'entrée dédiée gagne.
        let catalog = catalog();
        assert_eq!(
            catalog.resolve("lait de coco").unwrap().name,
            "lait de coco"
        );
        assert_eq!(catalog.resolve("lait demi-écrémé").unwrap().name, "lait");
    }

    #[test]
    fn resolve_tolerates_spelling_slips() {
        assert_eq!(catalog().resolve("échalotte").unwrap().name, "échalote");
    }

    #[test]
    fn resolve_gives_up_on_unknown_ingredients() {
        let catalog = catalog();
        assert!(catalog.resolve("farine de sarrasin").is_none());
        assert!(catalog.resolve("   ").is_none());
    }

    #[test]
    fn group_key_falls_back_on_the_canonical_key() {
        let catalog = catalog();
        // Rapproché : les deux formulations partagent la clé du dictionnaire.
        assert_eq!(
            catalog.group_key("Courgettes"),
            catalog.group_key("courgette")
        );
        // Inconnu : la clé canonique dédoublonne quand même les pluriels.
        assert_eq!(
            catalog.group_key("Farines T55"),
            catalog.group_key("farine t55")
        );
    }

    #[test]
    fn empty_catalog() {
        let catalog = ReferenceCatalog::new();
        assert!(catalog.is_empty());
        assert_eq!(catalog.len(), 0);
        assert!(catalog.resolve("courgette").is_none());
    }
}
