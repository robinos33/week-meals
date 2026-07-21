//! Test d'intégration du calendrier contre un **Postgres réel** (via l'API HTTP
//! complète). Marqué `#[ignore]` — nécessite `DATABASE_URL`. Lancer :
//!
//! ```sh
//! docker compose up -d
//! DATABASE_URL=postgres://weekmeals:weekmeals@localhost:5432/weekmeals \
//!     cargo test -p server --test meal_plan_flow -- --ignored
//! ```
//!
//! L'enjeu principal ici est la **FK composite** `(household_id, recipe_id)` :
//! elle garantit qu'on ne planifie qu'une recette DU foyer, et ne s'exerce que
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
use server::{app, init_session_store, pool, Config};
use tower::ServiceExt;
use uuid::Uuid;

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://weekmeals:weekmeals@localhost:5432/weekmeals".to_owned())
}

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
        .oneshot(json_request("POST", "/recipes", &body))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    json_body(response).await["id"].as_str().unwrap().to_owned()
}

/// Insère un foyer + une recette directement en base, et renvoie l'id recette.
/// Sert à fabriquer une recette d'un AUTRE foyer que celui de démo.
async fn seed_foreign_recipe(pool: &sqlx::PgPool, title: &str) -> Uuid {
    let household = Uuid::new_v4();
    let recipe = Uuid::new_v4();
    sqlx::query("insert into households (id, name) values ($1, 'Voisin')")
        .bind(household)
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("insert into recipes (id, household_id, title) values ($1, $2, $3)")
        .bind(recipe)
        .bind(household)
        .bind(title)
        .execute(pool)
        .await
        .unwrap();
    recipe
}

#[tokio::test]
#[ignore = "nécessite un Postgres (DATABASE_URL) — voir docstring"]
async fn meal_plan_flow() {
    let pool = pool(&database_url()).unwrap();
    sqlx::migrate!("../migrations")
        .run(&pool)
        .await
        .expect("migrations");
    let store = init_session_store(&pool).await.expect("store de sessions");
    // Mode public : l'extractor scope au foyer de démo sans session.
    std::env::set_var("AUTH_MODE", "disabled");
    let config = Config::from_env();
    let router = app(pool.clone(), store, &config);

    let marker = uuid::Uuid::new_v4().simple().to_string();
    let recipe_id = create_recipe(&router, &format!("Ratatouille {marker}")).await;

    // Créneaux propres au départ (le foyer de démo est partagé entre exécutions).
    for (date, slot) in [("2026-07-13", "dinner"), ("2026-07-14", "lunch")] {
        let _ = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/meal-plan/{date}/{slot}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    // Semaine vide au départ.
    let week = get_week(&router, "2026-07-13", "2026-07-19").await;
    assert_eq!(week.as_array().unwrap().len(), 0);

    // Placement, puis relecture.
    let placed = router
        .clone()
        .oneshot(json_request(
            "PUT",
            "/meal-plan/2026-07-13/dinner",
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
            "/meal-plan/2026-07-13/dinner",
            &serde_json::json!({ "recipe_id": other_id }),
        ))
        .await
        .unwrap();
    let week = get_week(&router, "2026-07-13", "2026-07-19").await;
    assert_eq!(week.as_array().unwrap().len(), 1);
    assert_eq!(week[0]["recipe_id"], other_id);

    // Le cœur du sujet : la recette d'un AUTRE foyer est refusée par la FK
    // composite — 404, jamais 500, et rien n'est planifié.
    let foreign_recipe = seed_foreign_recipe(&pool, &format!("Chez le voisin {marker}")).await;
    let refused = router
        .clone()
        .oneshot(json_request(
            "PUT",
            "/meal-plan/2026-07-14/lunch",
            &serde_json::json!({ "recipe_id": foreign_recipe }),
        ))
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::NOT_FOUND);

    // Une plage démesurée est refusée plutôt que de tout ramener.
    let too_wide = router
        .clone()
        .oneshot(get("/meal-plan?from=2026-01-01&to=9999-12-31"))
        .await
        .unwrap();
    assert_eq!(too_wide.status(), StatusCode::UNPROCESSABLE_ENTITY);

    // Vidage, puis créneau déjà vide : 404.
    let cleared = router
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/meal-plan/2026-07-13/dinner")
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
                .uri("/meal-plan/2026-07-13/dinner")
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
        .oneshot(get(&format!("/meal-plan?from={from}&to={to}")))
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
