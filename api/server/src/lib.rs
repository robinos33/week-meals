//! Crate `server` — couche HTTP (Axum) et composition de l'application.
//!
//! Le binaire monte ici les sous-routers exposés par chaque domaine. Le
//! routeur est construit par [`app`] afin que les tests d'intégration
//! puissent monter l'application sans ouvrir de socket réseau.

use axum::{routing::get, Json, Router};
use serde::Serialize;

/// Construit le routeur de l'application Week Meals.
///
/// Au fil des jalons, chaque domaine y greffe son sous-router, p. ex. :
/// `.merge(recipes::presentation::router())`.
pub fn app() -> Router {
    Router::new().route("/health", get(health))
}

/// Réponse du endpoint de santé.
#[derive(Serialize)]
struct Health {
    status: &'static str,
}

/// Endpoint de liveness : renvoie 200 tant que le process répond.
async fn health() -> Json<Health> {
    Json(Health { status: "ok" })
}
