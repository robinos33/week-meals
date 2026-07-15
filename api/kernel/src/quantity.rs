//! Value objects `Unit` et `Quantity` — grandeurs physiques manipulées par
//! les recettes et la liste de courses.
//!
//! Trois **dimensions** cohabitent : la masse (`g`, `kg`), le volume
//! (`ml`, `l`) et le comptage (`piece`). Chaque dimension possède une unité
//! de base (respectivement le gramme, le millilitre et la pièce) vers
//! laquelle on normalise avant tout calcul.

use std::fmt;

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

/// Unité d'une quantité d'ingrédient.
///
/// Ensemble fermé aligné sur le modèle métier (`plan.md`) : `g`, `kg`,
/// `ml`, `l`, `piece`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unit {
    /// Gramme.
    Gram,
    /// Kilogramme (= 1000 g).
    Kilogram,
    /// Millilitre.
    Milliliter,
    /// Litre (= 1000 ml).
    Liter,
    /// Pièce (unité de comptage).
    Piece,
}

impl Unit {
    /// Dimension physique de l'unité.
    #[must_use]
    pub fn dimension(self) -> Dimension {
        match self {
            Unit::Gram | Unit::Kilogram => Dimension::Mass,
            Unit::Milliliter | Unit::Liter => Dimension::Volume,
            Unit::Piece => Dimension::Count,
        }
    }

    /// Unité de base de la dimension de `self` (gramme, millilitre ou pièce).
    #[must_use]
    pub fn base(self) -> Unit {
        match self.dimension() {
            Dimension::Mass => Unit::Gram,
            Dimension::Volume => Unit::Milliliter,
            Dimension::Count => Unit::Piece,
        }
    }

    /// Facteur multiplicatif vers l'unité de base de la dimension.
    ///
    /// `Kilogram` → 1000 (1 kg = 1000 g), `Gram` → 1, etc.
    #[must_use]
    pub fn base_factor(self) -> f64 {
        match self {
            Unit::Gram | Unit::Milliliter | Unit::Piece => 1.0,
            Unit::Kilogram | Unit::Liter => 1000.0,
        }
    }

    /// Symbole d'affichage de l'unité.
    #[must_use]
    pub fn symbol(self) -> &'static str {
        match self {
            Unit::Gram => "g",
            Unit::Kilogram => "kg",
            Unit::Milliliter => "ml",
            Unit::Liter => "l",
            Unit::Piece => "piece",
        }
    }
}

impl fmt::Display for Unit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.symbol())
    }
}

/// Erreur de construction d'une [`Quantity`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantityError {
    /// Le montant est négatif.
    Negative,
    /// Le montant n'est pas un nombre fini (NaN ou infini).
    NotFinite,
}

impl fmt::Display for QuantityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            QuantityError::Negative => f.write_str("la quantité ne peut pas être négative"),
            QuantityError::NotFinite => f.write_str("la quantité doit être un nombre fini"),
        }
    }
}

impl std::error::Error for QuantityError {}

/// Quantité : un montant positif ou nul assorti d'une [`Unit`].
///
/// Value object immuable. Le montant est garanti fini et `>= 0` par
/// construction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quantity {
    amount: f64,
    unit: Unit,
}

impl Quantity {
    /// Construit une quantité en validant le montant (fini, `>= 0`).
    ///
    /// # Errors
    ///
    /// Renvoie [`QuantityError`] si `amount` est négatif ou non fini.
    pub fn new(amount: f64, unit: Unit) -> Result<Self, QuantityError> {
        if !amount.is_finite() {
            return Err(QuantityError::NotFinite);
        }
        if amount < 0.0 {
            return Err(QuantityError::Negative);
        }
        Ok(Self { amount, unit })
    }

    /// Montant dans l'unité de la quantité.
    #[must_use]
    pub fn amount(self) -> f64 {
        self.amount
    }

    /// Unité de la quantité.
    #[must_use]
    pub fn unit(self) -> Unit {
        self.unit
    }

    /// Dimension de la quantité (raccourci vers [`Unit::dimension`]).
    #[must_use]
    pub fn dimension(self) -> Dimension {
        self.unit.dimension()
    }

    /// Montant converti dans l'unité de base de sa dimension
    /// (grammes, millilitres ou pièces).
    #[must_use]
    pub fn in_base(self) -> f64 {
        self.amount * self.unit.base_factor()
    }
}

impl fmt::Display for Quantity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.amount, self.unit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dimensions_are_mapped_correctly() {
        assert_eq!(Unit::Gram.dimension(), Dimension::Mass);
        assert_eq!(Unit::Kilogram.dimension(), Dimension::Mass);
        assert_eq!(Unit::Milliliter.dimension(), Dimension::Volume);
        assert_eq!(Unit::Liter.dimension(), Dimension::Volume);
        assert_eq!(Unit::Piece.dimension(), Dimension::Count);
    }

    #[test]
    fn base_unit_of_each_dimension() {
        assert_eq!(Unit::Kilogram.base(), Unit::Gram);
        assert_eq!(Unit::Liter.base(), Unit::Milliliter);
        assert_eq!(Unit::Piece.base(), Unit::Piece);
    }

    #[test]
    fn base_factors_normalise_to_base_unit() {
        assert!((Unit::Kilogram.base_factor() - 1000.0).abs() < f64::EPSILON);
        assert!((Unit::Liter.base_factor() - 1000.0).abs() < f64::EPSILON);
        assert!((Unit::Gram.base_factor() - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn in_base_converts_amount() {
        let half_kg = Quantity::new(0.5, Unit::Kilogram).unwrap();
        assert!((half_kg.in_base() - 500.0).abs() < f64::EPSILON);

        let one_and_half_l = Quantity::new(1.5, Unit::Liter).unwrap();
        assert!((one_and_half_l.in_base() - 1500.0).abs() < f64::EPSILON);

        let three_pieces = Quantity::new(3.0, Unit::Piece).unwrap();
        assert!((three_pieces.in_base() - 3.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zero_is_a_valid_quantity() {
        let q = Quantity::new(0.0, Unit::Gram).unwrap();
        assert_eq!(q.amount(), 0.0);
    }

    #[test]
    fn negative_amount_is_rejected() {
        assert_eq!(
            Quantity::new(-1.0, Unit::Gram),
            Err(QuantityError::Negative)
        );
    }

    #[test]
    fn non_finite_amount_is_rejected() {
        assert_eq!(
            Quantity::new(f64::NAN, Unit::Gram),
            Err(QuantityError::NotFinite)
        );
        assert_eq!(
            Quantity::new(f64::INFINITY, Unit::Gram),
            Err(QuantityError::NotFinite)
        );
    }

    #[test]
    fn display_shows_symbol() {
        assert_eq!(Unit::Kilogram.to_string(), "kg");
        assert_eq!(
            Quantity::new(2.0, Unit::Piece).unwrap().to_string(),
            "2 piece"
        );
    }
}
