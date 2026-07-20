//! Identifiants forts, partagés entre domaines.
//!
//! Chaque identifiant est un newtype autour d'un [`Uuid`] : le compilateur
//! empêche alors de passer un `RecipeId` là où un `HouseholdId` est attendu.
//! Ces identifiants sont transverses (une recette est référencée par le
//! calendrier, une donnée est scopée à un foyer) — ils vivent donc dans le
//! `kernel` plutôt que dans un domaine (cf. ADR-0005).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Déclare un identifiant newtype autour d'un `Uuid`, avec les conversions et
/// les constructeurs usuels (`new` = génération aléatoire v4, `from`, `Display`).
macro_rules! id_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(Uuid);

        impl $name {
            /// Génère un nouvel identifiant aléatoire (UUID v4).
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            /// Expose l'`Uuid` sous-jacent (pour la persistance, p. ex.).
            #[must_use]
            pub fn as_uuid(&self) -> Uuid {
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

        impl From<$name> for Uuid {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Display::fmt(&self.0, f)
            }
        }
    };
}

id_type! {
    /// Identifiant d'un foyer : toutes les données applicatives y sont scopées.
    HouseholdId
}

id_type! {
    /// Identifiant d'un utilisateur (membre d'un foyer).
    UserId
}

id_type! {
    /// Identifiant d'une recette.
    RecipeId
}

id_type! {
    /// Identifiant d'un appareil enrôlé (porteur d'une passkey, cf. ADR-0006).
    DeviceId
}

id_type! {
    /// Identifiant d'une ligne de liste de courses.
    ShoppingItemId
}

/// Foyer de démonstration seedé par la migration `seed_demo_household`.
///
/// Cible du **mode public** (auth désactivée, cf. `AUTH_DISABLED`) et du **seed
/// CLI**. UUID fixe : contrat partagé avec la migration SQL — ne pas le changer
/// sans migration correspondante. Source unique, réutilisée par `auth` et `cli`.
pub const DEMO_HOUSEHOLD_ID: Uuid = Uuid::from_u128(0x0000_0000_0000_0000_0000_0000_0000_0001);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_uuid() {
        let uuid = Uuid::new_v4();
        let id = RecipeId::from(uuid);
        assert_eq!(id.as_uuid(), uuid);
        assert_eq!(Uuid::from(id), uuid);
    }

    #[test]
    fn new_yields_distinct_ids() {
        assert_ne!(HouseholdId::new(), HouseholdId::new());
    }
}
