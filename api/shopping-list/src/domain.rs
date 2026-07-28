//! Couche domaine de `shopping-list` : entités, value objects, traits de repository
//! et services purs. Aucune dépendance à SQLx/Axum (règle de convention).
//!
//! - [`reference`]  — dictionnaire des ingrédients ([`IngredientReference`],
//!   [`ReferenceCatalog`]).
//! - [`aisle`]      — catalogue fermé des rayons (le vocabulaire de classement).
//! - [`store`]      — magasins du foyer et ordre de visite de leurs rayons.
//! - [`matching`]   — rapprochement d'un nom libre au dictionnaire
//!   (normalisation, qualificatifs, proximité orthographique).
//! - [`conversion`] — service pur de conversion grammes → unités achetables
//!   (cœur métier, cf. `plan.md`).

pub mod aisle;
pub mod conversion;
pub mod list;
pub mod matching;
pub mod reference;
pub mod store;

pub use aisle::Aisle;
pub use conversion::{aggregate_purchases, PlannedIngredient, PurchaseItem};
pub use list::{
    CookedCountRecorder, PlannedIngredientsSource, ReferenceRepository, ShoppingItem,
    ShoppingListRepository,
};
pub use matching::canonical_key;
pub use reference::{IngredientReference, ReferenceCatalog};
pub use store::{Store, StoreError, StoreRepository};
