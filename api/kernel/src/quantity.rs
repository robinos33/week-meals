//! Value objects `Unit` et `Quantity`.
//!
//! Partagés par plusieurs domaines : une quantité d'ingrédient de recette et
//! une ligne de liste de courses utilisent les mêmes unités, et le service de
//! conversion grammes → unités (cœur métier) raisonne dessus. Ils vivent donc
//! dans le `kernel` (cf. ADR-0005).

use serde::{Deserialize, Serialize};

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
}
