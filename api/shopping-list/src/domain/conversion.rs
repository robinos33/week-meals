//! Service de domaine **pur** : conversion des grammages agrégés en unités
//! achetables. Cœur métier de Week Meals (`plan.md` § « Conversion grammes →
//! unités »). Zéro I/O, trivialement testable.
//!
//! Étapes :
//!
//! 1. **Agréger** les quantités d'un même ingrédient sur toutes les recettes
//!    planifiées (600 g + 300 g de courgettes → 900 g).
//! 2. **Convertir** via le référentiel : `900 g ÷ 250 g/pièce → 4 courgettes`
//!    (arrondi **supérieur**).
//! 3. Les ingrédients **comptables** (œufs, ail…) restent en pièces ; les
//!    **vracs** sans entrée au référentiel (farine, lait…) restent dans leur
//!    unité de base (g / mL).
//!
//! La sortie exprime les vracs dans l'unité de base de leur dimension
//! (gramme, millilitre, pièce) ; le confort d'affichage (g → kg, mL → L) est
//! du ressort de la couche présentation.

use kernel::{Dimension, Quantity, Unit};

use super::reference::{normalize_name, ReferenceCatalog};

/// Marge de tolérance retranchée avant l'arrondi supérieur, pour qu'une
/// division qui « tombe juste » en arithmétique réelle (ex. `500 / 250 = 2`)
/// ne soit pas faussement arrondie au-dessus à cause des erreurs flottantes.
const CEIL_EPSILON: f64 = 1e-6;

/// Un ingrédient tel qu'il apparaît dans une recette planifiée.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedIngredient {
    /// Nom de l'ingrédient (rapproché du référentiel après normalisation).
    pub name: String,
    /// Quantité requise par la recette.
    pub quantity: Quantity,
}

impl PlannedIngredient {
    /// Construit un ingrédient planifié.
    #[must_use]
    pub fn new(name: impl Into<String>, quantity: Quantity) -> Self {
        Self {
            name: name.into(),
            quantity,
        }
    }
}

/// Une ligne de la liste de courses : un ingrédient agrégé, prêt à acheter.
#[derive(Debug, Clone, PartialEq)]
pub struct PurchaseItem {
    /// Nom affiché (nom canonique du référentiel s'il existe, sinon le nom
    /// tel que planifié).
    pub name: String,
    /// Quantité à acheter (en pièces si l'ingrédient est référencé, sinon
    /// dans l'unité de base de sa dimension).
    pub quantity: Quantity,
    /// Catégorie de rayon, connue uniquement pour les ingrédients référencés.
    pub category: Option<String>,
}

/// Accumulateur par ingrédient, indexé sur le nom normalisé, dans l'ordre de
/// première apparition (sortie déterministe).
#[derive(Debug)]
struct Aggregate {
    /// Nom affiché : nom tel que rencontré la première fois.
    display_name: String,
    /// Masse cumulée, en grammes (unité de base masse).
    mass_g: f64,
    /// Volume cumulé, en millilitres (unité de base volume).
    volume_ml: f64,
    /// Comptage cumulé, en pièces.
    pieces: f64,
}

impl Aggregate {
    fn new(display_name: String) -> Self {
        Self {
            display_name,
            mass_g: 0.0,
            volume_ml: 0.0,
            pieces: 0.0,
        }
    }

    fn add(&mut self, quantity: Quantity) {
        match quantity.dimension() {
            Dimension::Mass => self.mass_g += quantity.in_base(),
            Dimension::Volume => self.volume_ml += quantity.in_base(),
            Dimension::Count => self.pieces += quantity.in_base(),
        }
    }
}

/// Agrège les ingrédients planifiés et les convertit en lignes de liste de
/// courses achetables.
///
/// Les quantités d'un même ingrédient (à la casse et aux espaces près) sont
/// cumulées. Un ingrédient **référencé** est rendu en pièces (arrondi
/// supérieur) ; un ingrédient **non référencé** (vrac) est rendu dans l'unité
/// de base de chaque dimension présente. Les lignes de quantité nulle sont
/// omises. L'ordre de sortie suit la première apparition de chaque
/// ingrédient.
#[must_use]
pub fn aggregate_purchases(
    planned: &[PlannedIngredient],
    catalog: &ReferenceCatalog,
) -> Vec<PurchaseItem> {
    // Regroupement ordonné : un Vec pour l'ordre, un index nom→position.
    let mut order: Vec<String> = Vec::new();
    let mut aggregates: std::collections::HashMap<String, Aggregate> =
        std::collections::HashMap::new();

    for ingredient in planned {
        let key = normalize_name(&ingredient.name);
        let aggregate = aggregates.entry(key.clone()).or_insert_with(|| {
            order.push(key.clone());
            Aggregate::new(ingredient.name.trim().to_string())
        });
        aggregate.add(ingredient.quantity);
    }

    let mut items = Vec::new();
    for key in order {
        let aggregate = &aggregates[&key];
        match catalog.find(&key) {
            Some(reference) => emit_referenced(&mut items, aggregate, reference),
            None => emit_bulk(&mut items, aggregate),
        }
    }
    items
}

/// Ingrédient présent au référentiel : tout se ramène à des pièces via le
/// poids moyen (les pièces déjà comptées font l'aller-retour à l'identique).
fn emit_referenced(
    items: &mut Vec<PurchaseItem>,
    aggregate: &Aggregate,
    reference: &super::reference::IngredientReference,
) {
    let avg = f64::from(reference.avg_weight_g);
    let convertible_g = aggregate.mass_g + aggregate.pieces * avg;

    if convertible_g > 0.0 {
        let pieces = ceil_to_pieces(convertible_g, avg);
        if pieces > 0 {
            items.push(PurchaseItem {
                name: reference.name.clone(),
                quantity: piece_quantity(pieces),
                category: Some(reference.category.clone()),
            });
        }
    }

    // Cas limite : un ingrédient pesé référencé reçu en volume ne peut pas
    // être converti en pièces (le référentiel est massique). Plutôt que de le
    // perdre, on le conserve en vrac volumique.
    if aggregate.volume_ml > 0.0 {
        items.push(PurchaseItem {
            name: reference.name.clone(),
            quantity: base_quantity(aggregate.volume_ml, Unit::Milliliter),
            category: Some(reference.category.clone()),
        });
    }
}

/// Ingrédient absent du référentiel (vrac) : conservé dans l'unité de base de
/// chaque dimension présente.
fn emit_bulk(items: &mut Vec<PurchaseItem>, aggregate: &Aggregate) {
    if aggregate.mass_g > 0.0 {
        items.push(PurchaseItem {
            name: aggregate.display_name.clone(),
            quantity: base_quantity(aggregate.mass_g, Unit::Gram),
            category: None,
        });
    }
    if aggregate.volume_ml > 0.0 {
        items.push(PurchaseItem {
            name: aggregate.display_name.clone(),
            quantity: base_quantity(aggregate.volume_ml, Unit::Milliliter),
            category: None,
        });
    }
    if aggregate.pieces > 0.0 {
        items.push(PurchaseItem {
            name: aggregate.display_name.clone(),
            quantity: base_quantity(aggregate.pieces, Unit::Piece),
            category: None,
        });
    }
}

/// Divise `total_base` (grammes) par `avg_weight_g` et arrondit **au-dessus**,
/// avec une tolérance flottante pour les divisions exactes.
fn ceil_to_pieces(total_base: f64, avg_weight_g: f64) -> u32 {
    let raw = total_base / avg_weight_g;
    let rounded = (raw - CEIL_EPSILON).ceil();
    if rounded <= 0.0 {
        0
    } else {
        rounded as u32
    }
}

/// Quantité en pièces (jamais invalide : entier positif).
fn piece_quantity(pieces: u32) -> Quantity {
    Quantity::new(f64::from(pieces), Unit::Piece)
        .expect("un entier de pièces est une quantité valide")
}

/// Quantité dans une unité de base à partir d'un montant garanti positif fini.
fn base_quantity(amount: f64, unit: Unit) -> Quantity {
    Quantity::new(amount, unit).expect("un montant agrégé positif fini est une quantité valide")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::reference::IngredientReference;

    /// Catalogue de test aligné sur `data/ingredients.yaml`.
    fn catalog() -> ReferenceCatalog {
        ReferenceCatalog::from_iter([
            IngredientReference::new("courgette", "legumes", 250, false),
            IngredientReference::new("aubergine", "legumes", 300, false),
            IngredientReference::new("œuf", "cremerie", 55, true),
            IngredientReference::new("gousse d'ail", "legumes", 7, true),
        ])
    }

    fn g(amount: f64) -> Quantity {
        Quantity::new(amount, Unit::Gram).unwrap()
    }
    fn kg(amount: f64) -> Quantity {
        Quantity::new(amount, Unit::Kilogram).unwrap()
    }
    fn piece(amount: f64) -> Quantity {
        Quantity::new(amount, Unit::Piece).unwrap()
    }
    fn ml(amount: f64) -> Quantity {
        Quantity::new(amount, Unit::Milliliter).unwrap()
    }

    #[test]
    fn aggregates_same_ingredient_across_recipes() {
        // 600 g + 300 g de courgettes → 900 g → ÷250 → 4 courgettes.
        let planned = [
            PlannedIngredient::new("courgette", g(600.0)),
            PlannedIngredient::new("courgette", g(300.0)),
        ];
        let items = aggregate_purchases(&planned, &catalog());

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "courgette");
        assert_eq!(items[0].quantity, piece(4.0));
        assert_eq!(items[0].category.as_deref(), Some("legumes"));
    }

    #[test]
    fn ceil_rounds_up_partial_pieces() {
        // 900 g / 250 = 3.6 → 4.
        let planned = [PlannedIngredient::new("courgette", g(900.0))];
        let items = aggregate_purchases(&planned, &catalog());
        assert_eq!(items[0].quantity, piece(4.0));
    }

    #[test]
    fn exact_division_does_not_round_up() {
        // 500 g / 250 = 2 pile → 2, pas 3.
        let planned = [PlannedIngredient::new("courgette", g(500.0))];
        let items = aggregate_purchases(&planned, &catalog());
        assert_eq!(items[0].quantity, piece(2.0));
    }

    #[test]
    fn just_over_boundary_rounds_up() {
        // 501 g / 250 = 2.004 → 3.
        let planned = [PlannedIngredient::new("courgette", g(501.0))];
        let items = aggregate_purchases(&planned, &catalog());
        assert_eq!(items[0].quantity, piece(3.0));
    }

    #[test]
    fn kilograms_are_normalised_before_conversion() {
        // 1,2 kg d'aubergines → 1200 g / 300 = 4.
        let planned = [PlannedIngredient::new("aubergine", kg(1.2))];
        let items = aggregate_purchases(&planned, &catalog());
        assert_eq!(items[0].quantity, piece(4.0));
    }

    #[test]
    fn mixed_units_of_referenced_ingredient_are_combined() {
        // 600 g + 1 pièce de courgette → 600 + 250 = 850 g / 250 = 3.4 → 4.
        let planned = [
            PlannedIngredient::new("courgette", g(600.0)),
            PlannedIngredient::new("courgette", piece(1.0)),
        ];
        let items = aggregate_purchases(&planned, &catalog());
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].quantity, piece(4.0));
    }

    #[test]
    fn countable_ingredient_stays_in_pieces() {
        // 3 œufs + 3 œufs → 6 œufs (round-trip exact via le poids moyen).
        let planned = [
            PlannedIngredient::new("œuf", piece(3.0)),
            PlannedIngredient::new("œuf", piece(3.0)),
        ];
        let items = aggregate_purchases(&planned, &catalog());
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "œuf");
        assert_eq!(items[0].quantity, piece(6.0));
        assert_eq!(items[0].category.as_deref(), Some("cremerie"));
    }

    #[test]
    fn countable_ingredient_with_small_avg_weight() {
        // 2 + 4 gousses d'ail → 6 (7 g/pièce, aller-retour exact).
        let planned = [
            PlannedIngredient::new("gousse d'ail", piece(2.0)),
            PlannedIngredient::new("gousse d'ail", piece(4.0)),
        ];
        let items = aggregate_purchases(&planned, &catalog());
        assert_eq!(items[0].quantity, piece(6.0));
    }

    #[test]
    fn bulk_ingredient_without_reference_stays_in_grams() {
        // Farine : pas d'entrée référentiel → reste en grammes, cumulée.
        let planned = [
            PlannedIngredient::new("farine", g(250.0)),
            PlannedIngredient::new("farine", kg(0.5)),
        ];
        let items = aggregate_purchases(&planned, &catalog());
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "farine");
        assert_eq!(items[0].quantity, g(750.0));
        assert_eq!(items[0].category, None);
    }

    #[test]
    fn bulk_liquid_stays_in_millilitres() {
        // Lait : vrac liquide → agrégé en mL (0,5 L + 200 mL = 700 mL).
        let planned = [
            PlannedIngredient::new("lait", Quantity::new(0.5, Unit::Liter).unwrap()),
            PlannedIngredient::new("lait", ml(200.0)),
        ];
        let items = aggregate_purchases(&planned, &catalog());
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].quantity, ml(700.0));
        assert_eq!(items[0].category, None);
    }

    #[test]
    fn name_matching_is_case_and_whitespace_insensitive() {
        let planned = [
            PlannedIngredient::new("Courgette", g(600.0)),
            PlannedIngredient::new("  courgette ", g(300.0)),
        ];
        let items = aggregate_purchases(&planned, &catalog());
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].quantity, piece(4.0));
    }

    #[test]
    fn output_uses_canonical_reference_name() {
        // Nom d'entrée « Courgette » mais sortie sur le nom canonique.
        let planned = [PlannedIngredient::new("Courgette", g(250.0))];
        let items = aggregate_purchases(&planned, &catalog());
        assert_eq!(items[0].name, "courgette");
    }

    #[test]
    fn bulk_output_preserves_first_seen_name() {
        let planned = [PlannedIngredient::new("Farine T55", g(250.0))];
        let items = aggregate_purchases(&planned, &catalog());
        assert_eq!(items[0].name, "Farine T55");
    }

    #[test]
    fn preserves_first_seen_order() {
        let planned = [
            PlannedIngredient::new("farine", g(100.0)),
            PlannedIngredient::new("courgette", g(250.0)),
            PlannedIngredient::new("lait", ml(100.0)),
        ];
        let items = aggregate_purchases(&planned, &catalog());
        let names: Vec<&str> = items.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["farine", "courgette", "lait"]);
    }

    #[test]
    fn empty_input_yields_empty_list() {
        let items = aggregate_purchases(&[], &catalog());
        assert!(items.is_empty());
    }

    #[test]
    fn empty_catalog_treats_everything_as_bulk() {
        let planned = [PlannedIngredient::new("courgette", g(600.0))];
        let items = aggregate_purchases(&planned, &ReferenceCatalog::new());
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].quantity, g(600.0));
        assert_eq!(items[0].category, None);
    }

    #[test]
    fn zero_quantity_referenced_yields_no_line() {
        let planned = [PlannedIngredient::new("courgette", g(0.0))];
        let items = aggregate_purchases(&planned, &catalog());
        assert!(items.is_empty());
    }

    #[test]
    fn zero_quantity_bulk_yields_no_line() {
        let planned = [PlannedIngredient::new("farine", g(0.0))];
        let items = aggregate_purchases(&planned, &catalog());
        assert!(items.is_empty());
    }

    #[test]
    fn tiny_quantity_rounds_up_to_one_piece() {
        // 10 g de courgette → moins d'une pièce mais > 0 → 1 (arrondi sup.).
        let planned = [PlannedIngredient::new("courgette", g(10.0))];
        let items = aggregate_purchases(&planned, &catalog());
        assert_eq!(items[0].quantity, piece(1.0));
    }

    #[test]
    fn bulk_ingredient_with_two_dimensions_emits_two_lines() {
        // Cas dégénéré : un vrac reçu en masse ET en volume → deux lignes,
        // masse puis volume (ordre déterministe).
        let planned = [
            PlannedIngredient::new("sirop", g(100.0)),
            PlannedIngredient::new("sirop", ml(200.0)),
        ];
        let items = aggregate_purchases(&planned, &ReferenceCatalog::new());
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].quantity, g(100.0));
        assert_eq!(items[1].quantity, ml(200.0));
    }

    #[test]
    fn referenced_ingredient_received_in_volume_is_kept_as_bulk() {
        // Incohérence de données : une courgette « en mL ». Non convertible
        // en pièces (référentiel massique) → conservée en volume plutôt que
        // perdue.
        let planned = [PlannedIngredient::new("courgette", ml(300.0))];
        let items = aggregate_purchases(&planned, &catalog());
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].quantity, ml(300.0));
        assert_eq!(items[0].category.as_deref(), Some("legumes"));
    }
}
