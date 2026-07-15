//! Identifiants transverses aux domaines.
//!
//! Ce sont des newtypes autour d'un `Uuid` (v4). Ils vivent dans le `kernel`
//! parce que plusieurs domaines s'y réfèrent : toutes les données sont scopées
//! au foyer (`HouseholdId`) et certaines portent leur auteur (`UserId`). Les
//! domaines dépendent du `kernel`, jamais l'inverse.

use std::fmt;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Déclare un identifiant newtype autour d'un `Uuid`, avec les conversions et
/// traits usuels. Évite de dupliquer la même cérémonie pour chaque type d'ID.
macro_rules! id_newtype {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            /// Génère un nouvel identifiant aléatoire (UUID v4).
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// Expose l'UUID sous-jacent (persistance, sérialisation).
            #[must_use]
            pub fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl From<Uuid> for $name {
            fn from(value: Uuid) -> Self {
                Self(value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }
    };
}

id_newtype! {
    /// Identifiant d'un foyer ([`Household`](../auth/index.html)). Toutes les
    /// données de l'app sont scopées à un foyer.
    HouseholdId
}

id_newtype! {
    /// Identifiant d'un utilisateur.
    UserId
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deux_ids_generes_sont_distincts() {
        assert_ne!(UserId::new(), UserId::new());
    }

    #[test]
    fn conversion_uuid_aller_retour() {
        let uuid = Uuid::new_v4();
        let id = HouseholdId::from(uuid);
        assert_eq!(id.as_uuid(), uuid);
    }
}
