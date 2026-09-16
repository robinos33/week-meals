//! Value objects `Unit` et `Quantity`.
//!
//! Partagés par plusieurs domaines : une quantité d'ingrédient de recette et
//! une ligne de liste de courses utilisent les mêmes unités, et le service de
//! conversion grammes → unités (cœur métier) raisonne dessus. Ils vivent donc
//! dans le `kernel` (cf. ADR-0005).
//!
//! Trois **dimensions** cohabitent : la masse (`g`, `kg`), le volume
//! (`ml`, `l`) et le comptage (`piece`). Chaque dimension possède une unité de
//! base (respectivement le gramme, le millilitre et la pièce) vers laquelle on
//! normalise avant tout calcul de conversion.

use serde::{Deserialize, Serialize};

/// Dimension physique d'une unité.
///
/// Deux quantités ne sont additionnables que si elles partagent la même
/// dimension (on n'ajoute pas des millilitres à des grammes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Dimension {
    /// Masse — unité de base : le gramme.
    Mass,
    /// Volume — unité de base : le millilitre.
    Volume,
    /// Comptage d'objets — unité de base : la pièce.
    Count,
}

/// Unité d'une quantité. Contrat public partagé avec le seed YAML et l'enum
/// SQL `unit` (cf. `data/recipes/*.yaml`, migration `recipes`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Unit {
    /// Grammes.
    G,
    /// Kilogrammes.
    Kg,
    /// Millilitres.
    Ml,
    /// Litres.
    L,
    /// Pièce (unité comptable : œuf, gousse d'ail…).
    Piece,
    /// Paquet (pâtes, biscuits, riz…).
    Paquet,
    /// Boîte (conserve, céréales…).
    Boite,
    /// Sachet (soupe, levure…).
    Sachet,
    /// Bouteille (huile, jus…).
    Bouteille,
    /// Pot (yaourt, confiture, moutarde…).
    Pot,
    /// Botte (radis, persil, asperges…).
    Botte,
    /// Barquette (fraises, champignons…).
    Barquette,
    /// Tranche (jambon, fromage…).
    Tranche,
}

impl Unit {
    /// Représentation textuelle canonique (celle du seed et de l'enum SQL).
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Unit::G => "g",
            Unit::Kg => "kg",
            Unit::Ml => "ml",
            Unit::L => "l",
            Unit::Piece => "piece",
            Unit::Paquet => "paquet",
            Unit::Boite => "boite",
            Unit::Sachet => "sachet",
            Unit::Bouteille => "bouteille",
            Unit::Pot => "pot",
            Unit::Botte => "botte",
            Unit::Barquette => "barquette",
            Unit::Tranche => "tranche",
        }
    }

    /// Dimension physique de l'unité.
    #[must_use]
    pub fn dimension(self) -> Dimension {
        match self {
            Unit::G | Unit::Kg => Dimension::Mass,
            Unit::Ml | Unit::L => Dimension::Volume,
            Unit::Piece
            | Unit::Paquet
            | Unit::Boite
            | Unit::Sachet
            | Unit::Bouteille
            | Unit::Pot
            | Unit::Botte
            | Unit::Barquette
            | Unit::Tranche => Dimension::Count,
        }
    }

    /// Unité de base de la dimension de `self` (gramme, millilitre ou
    /// elle-même pour un comptage : les unités de comptage ne se convertissent
    /// pas entre elles, cf. [`Unit::is_summable_with`]).
    #[must_use]
    pub fn base(self) -> Unit {
        match self.dimension() {
            Dimension::Mass => Unit::G,
            Dimension::Volume => Unit::Ml,
            Dimension::Count => self,
        }
    }

    /// Facteur multiplicatif vers l'unité de base de la dimension.
    ///
    /// `Kg` → 1000 (1 kg = 1000 g), `G` → 1, etc.
    #[must_use]
    pub fn base_factor(self) -> f64 {
        match self {
            Unit::Kg | Unit::L => 1000.0,
            Unit::G
            | Unit::Ml
            | Unit::Piece
            | Unit::Paquet
            | Unit::Boite
            | Unit::Sachet
            | Unit::Bouteille
            | Unit::Pot
            | Unit::Botte
            | Unit::Barquette
            | Unit::Tranche => 1.0,
        }
    }

    /// Deux unités se cumulent-elles nativement (mêmes lignes, même « case ») ?
    ///
    /// Les unités continues (masse, volume) se convertissent librement au sein
    /// de leur dimension (`500 g + 0,5 kg`). Un comptage n'a en revanche de
    /// sens que pour l'unité exacte : un paquet et une boîte ne sont pas
    /// interchangeables, contrairement à `Piece`, qui reste seule dans son cas
    /// d'usage historique (œufs, gousses d'ail…).
    #[must_use]
    pub fn is_summable_with(self, other: Unit) -> bool {
        match self.dimension() {
            Dimension::Count => self == other,
            Dimension::Mass | Dimension::Volume => self.dimension() == other.dimension(),
        }
    }
}

impl std::fmt::Display for Unit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Erreur de construction d'une [`Quantity`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum QuantityError {
    /// Le montant doit être strictement positif et fini.
    #[error("quantity amount must be a finite value greater than zero")]
    NonPositiveAmount,
}

/// Une quantité : un montant strictement positif associé à une [`Unit`].
///
/// L'invariant « montant > 0 » (aligné sur la contrainte SQL
/// `check (quantity > 0)`) est garanti à la construction ; les champs ne sont
/// donc pas mutables directement.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Quantity {
    amount: f64,
    unit: Unit,
}

impl Quantity {
    /// Construit une quantité en validant l'invariant `amount > 0` (fini).
    ///
    /// # Errors
    /// Renvoie [`QuantityError::NonPositiveAmount`] si `amount` est nul,
    /// négatif, `NaN` ou infini.
    pub fn new(amount: f64, unit: Unit) -> Result<Self, QuantityError> {
        if !amount.is_finite() || amount <= 0.0 {
            return Err(QuantityError::NonPositiveAmount);
        }
        Ok(Self { amount, unit })
    }

    /// Le montant (toujours strictement positif).
    #[must_use]
    pub fn amount(&self) -> f64 {
        self.amount
    }

    /// L'unité.
    #[must_use]
    pub fn unit(&self) -> Unit {
        self.unit
    }

    /// Dimension de la quantité (raccourci vers [`Unit::dimension`]).
    #[must_use]
    pub fn dimension(&self) -> Dimension {
        self.unit.dimension()
    }

    /// Montant converti dans l'unité de base de sa dimension
    /// (grammes, millilitres ou pièces).
    #[must_use]
    pub fn in_base(&self) -> f64 {
        self.amount * self.unit.base_factor()
    }
}

impl std::fmt::Display for Quantity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.amount, self.unit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_valid_quantity() {
        let q = Quantity::new(600.0, Unit::G).unwrap();
        assert_eq!(q.amount(), 600.0);
        assert_eq!(q.unit(), Unit::G);
    }

    #[test]
    fn rejects_non_positive_or_non_finite_amounts() {
        assert_eq!(
            Quantity::new(0.0, Unit::G),
            Err(QuantityError::NonPositiveAmount)
        );
        assert_eq!(
            Quantity::new(-1.0, Unit::L),
            Err(QuantityError::NonPositiveAmount)
        );
        assert_eq!(
            Quantity::new(f64::NAN, Unit::G),
            Err(QuantityError::NonPositiveAmount)
        );
        assert_eq!(
            Quantity::new(f64::INFINITY, Unit::G),
            Err(QuantityError::NonPositiveAmount)
        );
    }

    #[test]
    fn unit_serializes_to_seed_spelling() {
        assert_eq!(serde_json::to_string(&Unit::Piece).unwrap(), "\"piece\"");
        assert_eq!(serde_json::from_str::<Unit>("\"kg\"").unwrap(), Unit::Kg);
        assert_eq!(Unit::Ml.as_str(), "ml");
    }

    #[test]
    fn dimensions_are_mapped_correctly() {
        assert_eq!(Unit::G.dimension(), Dimension::Mass);
        assert_eq!(Unit::Kg.dimension(), Dimension::Mass);
        assert_eq!(Unit::Ml.dimension(), Dimension::Volume);
        assert_eq!(Unit::L.dimension(), Dimension::Volume);
        assert_eq!(Unit::Piece.dimension(), Dimension::Count);
    }

    #[test]
    fn base_unit_of_each_dimension() {
        assert_eq!(Unit::Kg.base(), Unit::G);
        assert_eq!(Unit::L.base(), Unit::Ml);
        assert_eq!(Unit::Piece.base(), Unit::Piece);
        assert_eq!(Unit::Boite.base(), Unit::Boite);
    }

    #[test]
    fn packaging_units_have_their_own_canonical_spelling_and_dimension() {
        assert_eq!(Unit::Paquet.as_str(), "paquet");
        assert_eq!(Unit::Boite.as_str(), "boite");
        assert_eq!(Unit::Sachet.as_str(), "sachet");
        assert_eq!(Unit::Bouteille.as_str(), "bouteille");
        assert_eq!(Unit::Pot.as_str(), "pot");
        assert_eq!(Unit::Botte.as_str(), "botte");
        assert_eq!(Unit::Barquette.as_str(), "barquette");
        assert_eq!(Unit::Tranche.as_str(), "tranche");
        for unit in [
            Unit::Paquet,
            Unit::Boite,
            Unit::Sachet,
            Unit::Bouteille,
            Unit::Pot,
            Unit::Botte,
            Unit::Barquette,
            Unit::Tranche,
        ] {
            assert_eq!(unit.dimension(), Dimension::Count);
            assert!((unit.base_factor() - 1.0).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn mass_and_volume_units_are_summable_across_their_dimension() {
        assert!(Unit::G.is_summable_with(Unit::Kg));
        assert!(Unit::L.is_summable_with(Unit::Ml));
    }

    #[test]
    fn count_units_are_only_summable_with_the_exact_same_unit() {
        assert!(Unit::Piece.is_summable_with(Unit::Piece));
        assert!(Unit::Boite.is_summable_with(Unit::Boite));
        assert!(!Unit::Boite.is_summable_with(Unit::Paquet));
        assert!(!Unit::Piece.is_summable_with(Unit::Boite));
    }

    #[test]
    fn base_factors_normalise_to_base_unit() {
        assert!((Unit::Kg.base_factor() - 1000.0).abs() < f64::EPSILON);
        assert!((Unit::L.base_factor() - 1000.0).abs() < f64::EPSILON);
        assert!((Unit::G.base_factor() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn in_base_converts_amount() {
        let half_kg = Quantity::new(0.5, Unit::Kg).unwrap();
        assert!((half_kg.in_base() - 500.0).abs() < f64::EPSILON);

        let one_and_half_l = Quantity::new(1.5, Unit::L).unwrap();
        assert!((one_and_half_l.in_base() - 1500.0).abs() < f64::EPSILON);

        let three_pieces = Quantity::new(3.0, Unit::Piece).unwrap();
        assert!((three_pieces.in_base() - 3.0).abs() < f64::EPSILON);
    }
}
