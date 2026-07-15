//! Couche domaine de `shopping-list` : entités, value objects, traits de repository
//! et services purs. Aucune dépendance à SQLx/Axum (règle de convention).
//!
//! - [`reference`]  — référentiel des poids moyens ([`IngredientReference`],
//!   [`ReferenceCatalog`]).
//! - [`conversion`] — service pur de conversion grammes → unités achetables
//!   (cœur métier, cf. `plan.md`).

pub mod conversion;
pub mod reference;

pub use conversion::{aggregate_purchases, PlannedIngredient, PurchaseItem};
pub use reference::{IngredientReference, ReferenceCatalog};
