//! Couche application de `shopping-list` : use cases.
//!
//! Chaque use case suit le pattern Command/Query + Handler + Response ; un
//! Handler retourne toujours un objet Response, jamais une exception qui
//! remonterait à la présentation.

/// Écritures : génération depuis le calendrier, ajout / édition / suppression.
pub mod commands;
/// Lectures : la liste courante du foyer.
pub mod queries;
