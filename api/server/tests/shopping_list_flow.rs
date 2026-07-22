//! Test d'intégration de la génération de liste de courses et du compteur
//! « cuisiné X fois » (#58), sur un fichier SQLite jetable (cf. ADR-0008) :
//!
//! ```sh
//! cargo test -p server --test shopping_list_flow
//! ```
//!
//! Le compteur est la partie du SQL qui a le plus changé en quittant Postgres :
//! une CTE modificatrice (`with … as (update … returning …)`, impossible en
//! SQLite) est devenue deux ordres dans une transaction, dont l'ordre relatif
//! porte toute la correction. Ce test l'exerce sur ses trois cas :
//! une recette planifiée deux fois compte deux fois, régénérer ne recompte pas,
//! et un créneau ajouté après coup compte une fois de plus.
//!
//! Mode public (`AUTH_MODE=disabled`, foyer de démo), comme les autres flux :
//! les cérémonies WebAuthn ne sont pas rejouables sans authentificateur.

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

/// Crée une recette et renvoie son identifiant.
async fn create_recipe(router: &Router, title: &str) -> String {
    let body = serde_json::json!({
        "title": title,
        "ingredients": [{ "name": "courgette", "amount": 600.0, "unit": "g" }],
        "steps": ["Émincer."]
    });
    let response = router
        .clone()
        .oneshot(json_request("POST", "/api/recipes", &body))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    json_body(response).await["id"].as_str().unwrap().to_owned()
}

/// Pose une recette sur un créneau du calendrier.
async fn plan(router: &Router, date: &str, slot: &str, recipe_id: &str) {
    let response = router
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/api/meal-plan/{date}/{slot}"),
            &serde_json::json!({ "recipe_id": recipe_id }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

/// Génère la liste de courses sur la plage donnée.
async fn generate(router: &Router, from: &str, to: &str) -> Value {
    let response = router
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/shopping-list/generate",
            &serde_json::json!({ "from": from, "to": to }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await
}

/// Lit le compteur « cuisiné X fois » d'une recette.
async fn cooked_count(router: &Router, recipe_id: &str) -> u64 {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/recipes/{recipe_id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await["cooked_count"].as_u64().unwrap()
}

#[tokio::test]
async fn generating_the_list_counts_cooked_recipes_once_per_slot() {
    let db = common::temp_database().await;
    let pool = db.pool.clone();
    let store = init_session_store(&pool).await.expect("store de sessions");
    std::env::set_var("AUTH_MODE", "disabled");
    let config = Config::from_env();
    let router = app(pool, store, &config);

    let ratatouille = create_recipe(&router, "Ratatouille").await;
    let tarte = create_recipe(&router, "Tarte aux courgettes").await;

    // La ratatouille occupe deux créneaux de la semaine, la tarte un seul.
    plan(&router, "2026-07-13", "dinner", &ratatouille).await;
    plan(&router, "2026-07-14", "lunch", &ratatouille).await;
    plan(&router, "2026-07-15", "dinner", &tarte).await;

    let items = generate(&router, "2026-07-13", "2026-07-19").await;
    // 600 g × 3 créneaux, agrégés en une seule ligne de courgette.
    let items = items.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], "courgette");
    assert_eq!(items[0]["amount"], 1800.0);

    // Deux créneaux ⇒ deux fois cuisinée.
    assert_eq!(cooked_count(&router, &ratatouille).await, 2);
    assert_eq!(cooked_count(&router, &tarte).await, 1);

    // Régénérer la même semaine ne recompte rien : c'est la raison d'être de
    // `counted_at`, et le point que l'ordre des deux `update` doit préserver.
    generate(&router, "2026-07-13", "2026-07-19").await;
    assert_eq!(cooked_count(&router, &ratatouille).await, 2);
    assert_eq!(cooked_count(&router, &tarte).await, 1);

    // Un créneau ajouté en cours de semaine, lui, compte à la régénération.
    plan(&router, "2026-07-16", "dinner", &tarte).await;
    generate(&router, "2026-07-13", "2026-07-19").await;
    assert_eq!(cooked_count(&router, &ratatouille).await, 2);
    assert_eq!(cooked_count(&router, &tarte).await, 2);
}

#[tokio::test]
async fn replacing_a_slot_makes_the_new_recipe_countable_again() {
    let db = common::temp_database().await;
    let pool = db.pool.clone();
    let store = init_session_store(&pool).await.expect("store de sessions");
    std::env::set_var("AUTH_MODE", "disabled");
    let config = Config::from_env();
    let router = app(pool, store, &config);

    let ratatouille = create_recipe(&router, "Ratatouille").await;
    let tarte = create_recipe(&router, "Tarte aux courgettes").await;

    plan(&router, "2026-07-13", "dinner", &ratatouille).await;
    generate(&router, "2026-07-13", "2026-07-19").await;
    assert_eq!(cooked_count(&router, &ratatouille).await, 1);

    // Changer la recette d'un créneau déjà compté remet `counted_at` à null :
    // la nouvelle recette doit pouvoir être comptée…
    plan(&router, "2026-07-13", "dinner", &tarte).await;
    generate(&router, "2026-07-13", "2026-07-19").await;
    assert_eq!(cooked_count(&router, &tarte).await, 1);
    // … sans que l'ancienne ne perde ni ne gagne quoi que ce soit.
    assert_eq!(cooked_count(&router, &ratatouille).await, 1);

    // Reposer la même recette sur le créneau ne le rouvre pas au comptage.
    plan(&router, "2026-07-13", "dinner", &tarte).await;
    generate(&router, "2026-07-13", "2026-07-19").await;
    assert_eq!(cooked_count(&router, &tarte).await, 1);
}
