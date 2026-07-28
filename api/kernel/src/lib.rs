//! Crate `kernel` — noyau partagé, pur.
//!
//! Types transverses aux domaines : value objects communs ([`Quantity`],
//! [`Unit`], [`Dimension`]), identifiants ([`HouseholdId`], [`RecipeId`]…) et
//! erreurs partagées ([`RepositoryError`]).
//!
//! `kernel` n'a aucune dépendance d'infrastructure et ne dépend d'aucun
//! domaine ; ce sont les domaines qui dépendent de `kernel`. Le contenu est
//! enrichi au fil des jalons.

mod ids;
mod quantity;
mod repository;

pub use ids::{
    DeviceId, HouseholdId, RecipeId, ShoppingItemId, StoreId, UserId, DEMO_HOUSEHOLD_ID,
};
pub use quantity::{Dimension, Quantity, QuantityError, Unit};
pub use repository::RepositoryError;

/// Nombre de personnes par défaut d'une recette et d'un créneau. Base
/// historique de toutes les recettes ; dénominateur de la mise à l'échelle de
/// la liste de courses (cf. migration `recipe_meal_servings`).
pub const DEFAULT_SERVINGS: u32 = 2;
