//! Couche application de `meal-plan` : use cases.
//!
//! Chaque use case suit le pattern Command/Query + Handler + Response ; un
//! Handler retourne toujours un objet Response, jamais une exception qui
//! remonterait à la présentation.

/// Écritures : placer / retirer une recette sur un créneau.
pub mod commands;
/// Lectures : lire la semaine.
pub mod queries;
