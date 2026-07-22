//! Test d'intégration du calendrier contre une **vraie base** (via l'API HTTP
//! complète), sur un fichier SQLite jetable (cf. ADR-0008) :
//!
//! ```sh
//! cargo test -p server --test meal_plan_flow
//! ```
//!
//! L'enjeu principal ici est le **scope au foyer** : on ne planifie qu'une
//! recette DU foyer. La FK composite `(household_id, recipe_id)` le garantit en
//! base, mais c'est le contrôle explicite du repository qui produit le 404
//! (SQLite ne nomme pas la contrainte violée) — d'où l'intérêt de l'éprouver
//! contre une vraie base.
//!
//! L'authentification par passkeys (ADR-0006) n'étant pas rejouable sans
//! authentificateur, le test s'exécute en **mode public** (`AUTH_MODE=disabled`,
//! foyer de démo). La recette « étrangère » est insérée directement dans un
//! autre foyer, ce que le mode public ne permet pas via l'API.

use axum::body::Body;
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::Value;
use server::{app, init_session_store, Config};
use tower::ServiceExt;
use uuid::Uuid;

mod common;

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// Crée une recette via l'API (dans le foyer de démo) et renvoie son identifiant.
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

/// Insère un foyer + une recette directement en base, et renvoie l'id recette.
/// Sert à fabriquer une recette d'un AUTRE foyer que celui de démo.
async fn seed_foreign_recipe(pool: &sqlx::SqlitePool, title: &str) -> Uuid {
    let household = Uuid::new_v4();
    let recipe = Uuid::new_v4();
    sqlx::query("insert into households (id, name) values (?, 'Voisin')")
        .bind(household)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("insert into recipes (id, household_id, title, title_norm) values (?, ?, ?, ?)")
        .bind(recipe)
        .bind(household)
        .bind(title)
        .bind(title.to_lowercase())
        .execute(pool)
        .await
        .unwrap();
    recipe
}

#[tokio::test]
async fn meal_plan_flow() {
    let db = common::temp_database().await;
    let pool = db.pool.clone();
    let store = init_session_store(&pool).await.expect("store de sessions");
    // Mode public : l'extractor scope au foyer de démo sans session.
    std::env::set_var("AUTH_MODE", "disabled");
    let config = Config::from_env();
    let router = app(pool.clone(), store, &config);

    let marker = uuid::Uuid::new_v4().simple().to_string();
    let recipe_id = create_recipe(&router, &format!("Ratatouille {marker}")).await;

    // Semaine vide au départ (base neuve).
    let week = get_week(&router, "2026-07-13", "2026-07-19").await;
    assert_eq!(week.as_array().unwrap().len(), 0);

    // Placement, puis relecture.
    let placed = router
        .clone()
        .oneshot(json_request(
            "PUT",
            "/api/meal-plan/2026-07-13/dinner",
            &serde_json::json!({ "recipe_id": recipe_id }),
        ))
        .await
        .unwrap();
    assert_eq!(placed.status(), StatusCode::NO_CONTENT);

    let week = get_week(&router, "2026-07-13", "2026-07-19").await;
    let slots = week.as_array().unwrap();
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0]["slot"], "dinner");
    assert_eq!(slots[0]["recipe_id"], recipe_id);

    // Replacer sur le même créneau remplace (upsert), sans doublon.
    let other_id = create_recipe(&router, &format!("Tarte aux pommes {marker}")).await;
    router
        .clone()
        .oneshot(json_request(
            "PUT",
            "/api/meal-plan/2026-07-13/dinner",
            &serde_json::json!({ "recipe_id": other_id }),
        ))
        .await
        .unwrap();
    let week = get_week(&router, "2026-07-13", "2026-07-19").await;
    assert_eq!(week.as_array().unwrap().len(), 1);
    assert_eq!(week[0]["recipe_id"], other_id);

    // Le cœur du sujet : la recette d'un AUTRE foyer est refusée — 404, jamais
    // 500, et rien n'est planifié.
    let foreign_recipe = seed_foreign_recipe(&pool, &format!("Chez le voisin {marker}")).await;
    let refused = router
        .clone()
        .oneshot(json_request(
            "PUT",
            "/api/meal-plan/2026-07-14/lunch",
            &serde_json::json!({ "recipe_id": foreign_recipe }),
        ))
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::NOT_FOUND);

    // Une plage démesurée est refusée plutôt que de tout ramener.
    let too_wide = router
        .clone()
        .oneshot(get("/api/meal-plan?from=2026-01-01&to=9999-12-31"))
        .await
        .unwrap();
    assert_eq!(too_wide.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Vidage, puis créneau déjà vide : 404.
    let cleared = router
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/meal-plan/2026-07-13/dinner")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cleared.status(), StatusCode::NO_CONTENT);

    let again = router
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/meal-plan/2026-07-13/dinner")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(again.status(), StatusCode::NOT_FOUND);
}

/// Lit la semaine et renvoie le corps JSON.
async fn get_week(router: &Router, from: &str, to: &str) -> Value {
    let response = router
        .clone()
        .oneshot(get(&format!("/api/meal-plan?from={from}&to={to}")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await
}

/// Requête GET simple.
fn get(uri: &str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

/// Construit une requête JSON (mode public : sans cookie).
fn json_request(method: &str, uri: &str, body: &Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}
