//! Test d'intégration du endpoint de santé — assemble l'application avec un
//! pool paresseux (aucune connexion réseau tant qu'aucune requête ne touche la
//! base). Le endpoint `/health` ne touche ni la base ni la session.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use server::{app, pool, Config};
use tower::ServiceExt; // pour `oneshot`
use tower_sessions_sqlx_store::SqliteStore;

#[tokio::test]
async fn health_returns_200() {
    // Base en mémoire : `/health` ne l'ouvre jamais, elle n'existe que pour
    // satisfaire la signature de `app`.
    let pool = pool("sqlite::memory:").unwrap();
    let store = SqliteStore::new(pool.clone());
    let config = Config::from_env();

    let response = app(pool, store, &config)
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
