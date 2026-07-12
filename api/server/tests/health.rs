//! Test d'intégration du endpoint de santé — amorce le harnais de tests HTTP.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use server::app;
use tower::ServiceExt; // pour `oneshot`

#[tokio::test]
async fn health_returns_200() {
    let response = app()
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
