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
    /// Identifiant d'un lien d'invitation.
    InvitationId
}

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
