//! Couche application de `auth` : use cases.
//!
//! Chaque use case suit le pattern Command/Query + Handler + Response ; un
//! Handler retourne toujours un objet Response, jamais une exception qui
//! remonterait à la présentation.

/// Écritures : Command + Handler + Response.
pub mod commands {}

/// Lectures : Query + Handler + Response.
pub mod queries {}
