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

pub use ids::{HouseholdId, InvitationId, RecipeId, ShoppingItemId, UserId, DEMO_HOUSEHOLD_ID};
pub use quantity::{Dimension, Quantity, QuantityError, Unit};
pub use repository::RepositoryError;
