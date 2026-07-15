//! Crate `kernel` — noyau partagé, pur.
//!
//! Types transverses aux domaines : value objects communs ([`Quantity`],
//! [`Unit`]), identifiants ([`HouseholdId`], [`RecipeId`]…) et erreurs
//! partagées ([`RepositoryError`]).
//!
//! `kernel` n'a aucune dépendance d'infrastructure et ne dépend d'aucun
//! domaine ; ce sont les domaines qui dépendent de `kernel`. Le contenu est
//! enrichi au fil des jalons.

mod ids;
mod quantity;
mod repository;

pub use ids::{HouseholdId, RecipeId};
pub use quantity::{Quantity, QuantityError, Unit};
pub use repository::RepositoryError;
