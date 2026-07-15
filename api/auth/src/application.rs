//! Couche application de `auth` : use cases.
//!
//! Chaque use case suit le pattern Command/Query + Handler + Response ; un
//! Handler retourne toujours un objet Response, jamais une exception qui
//! remonterait à la présentation.

/// Écritures / actions : Command + Handler + Response.
pub mod commands;
