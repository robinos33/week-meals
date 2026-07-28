//! Test d'intégration des magasins et de l'ordre de visite de leurs rayons,
//! sur un fichier SQLite jetable (cf. ADR-0008) :
//!
//! ```sh
//! cargo test -p server --test stores_flow
//! ```
//!
//! Ce qui se joue ici est la promesse du tri en magasin : un magasin naît avec
//! l'ordre de rayons par défaut, on le rectifie une fois pour toutes, et il
//! doit se relire **exactement** dans cet ordre — c'est lui qui décide de
//! l'ordre des sections de la liste de courses.
//!
//! Mode public (`AUTH_MODE=disabled`, foyer de démo), comme les autres flux.

use axum::body::Body;
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::Value;
use server::{app, init_session_store, Config};
use tower::ServiceExt;

mod common;

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

fn json_request(method: &str, uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn get(uri: &str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

/// Routeur sur une base neuve. La base temporaire est renvoyée avec lui : la
/// libérer effacerait le fichier sous les pieds du routeur.
async fn router() -> (Router, common::TempDatabase) {
    let db = common::temp_database().await;
    let store = init_session_store(&db.pool)
        .await
        .expect("store de sessions");
    std::env::set_var("AUTH_MODE", "disabled");
    let config = Config::from_env();
    let router = app(db.pool.clone(), store, &config);
    (router, db)
}

/// Crée un magasin et renvoie sa représentation JSON.
async fn create_store(router: &Router, name: &str) -> Value {
    let response = router
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/stores",
            &serde_json::json!({ "name": name }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    json_body(response).await
}

/// Slugs des rayons d'un magasin JSON.
fn aisles(store: &Value) -> Vec<String> {
    store["aisles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|slug| slug.as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn a_new_store_starts_from_the_catalogue_order() {
    let (router, _db) = router().await;

    let catalogue = json_body(router.clone().oneshot(get("/api/aisles")).await.unwrap()).await;
    let expected: Vec<String> = catalogue
        .as_array()
        .unwrap()
        .iter()
        .map(|aisle| aisle["slug"].as_str().unwrap().to_owned())
        .collect();
    assert!(expected.contains(&"epicerie-salee".to_owned()));

    let store = create_store(&router, "Super U des Halles").await;
    assert_eq!(store["name"], "Super U des Halles");
    assert_eq!(aisles(&store), expected);
}

#[tokio::test]
async fn the_visit_order_survives_a_reload() {
    let (router, _db) = router().await;
    let store = create_store(&router, "Lidl").await;
    let id = store["id"].as_str().unwrap();

    // Chez celui-ci, on commence par les surgelés et on finit par les fruits.
    let response = router
        .clone()
        .oneshot(json_request(
            "PATCH",
            &format!("/api/stores/{id}"),
            &serde_json::json!({ "aisles": ["surgeles", "boissons"] }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let reloaded = json_body(router.clone().oneshot(get("/api/stores")).await.unwrap()).await;
    let order = aisles(&reloaded[0]);
    assert_eq!(order[0], "surgeles");
    assert_eq!(order[1], "boissons");
    // Les rayons non cités suivent : rien ne se perd en route.
    assert_eq!(order.len(), aisles(&store).len());
}

#[tokio::test]
async fn a_store_is_renamed_then_deleted() {
    let (router, _db) = router().await;
    let id = create_store(&router, "Carrefour")
        .await
        .get("id")
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned();

    let renamed = json_body(
        router
            .clone()
            .oneshot(json_request(
                "PATCH",
                &format!("/api/stores/{id}"),
                &serde_json::json!({ "name": "Carrefour Market" }),
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(renamed["name"], "Carrefour Market");

    let deleted = router
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/stores/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);

    let remaining = json_body(router.oneshot(get("/api/stores")).await.unwrap()).await;
    assert_eq!(remaining.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn a_store_without_a_name_is_refused() {
    let (router, _db) = router().await;
    let response = router
        .oneshot(json_request(
            "POST",
            "/api/stores",
            &serde_json::json!({ "name": "   " }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
}
