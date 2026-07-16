//! Test d'intégration du calendrier contre un **Postgres réel** (via l'API HTTP
//! complète, cookie de session inclus). Marqué `#[ignore]` — nécessite
//! `DATABASE_URL`. Lancer :
//!
//! ```sh
//! docker compose up -d
//! DATABASE_URL=postgres://weekmeals:weekmeals@localhost:5432/weekmeals \
//!     cargo test -p server --test meal_plan_flow -- --ignored
//! ```
//!
//! L'enjeu principal ici est la **FK composite** `(household_id, recipe_id)` :
//! elle est ce qui garantit qu'on ne planifie qu'une recette DU foyer, et elle
//! ne s'exerce que contre une vraie base.

use auth::domain::household::{Household, HouseholdName};
use auth::domain::password::{Argon2Hasher, Password, PasswordHasher};
use auth::domain::user::{User, Username};
use auth::domain::{HouseholdRepository, UserRepository};
use auth::infrastructure::{SqlxHouseholdRepository, SqlxUserRepository};
use axum::body::Body;
use axum::http::header::{CONTENT_TYPE, COOKIE, SET_COOKIE};
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::Value;
use server::{app, init_session_store, pool, Config};
use tower::ServiceExt;

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://weekmeals:weekmeals@localhost:5432/weekmeals".to_owned())
}

async fn json_body(response: axum::response::Response) -> Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

/// Seed d'un foyer + utilisateur, puis login : renvoie le cookie de session.
async fn sign_in(router: &Router, pool: &sqlx::PgPool, prefix: &str) -> String {
    let username = format!(
        "{prefix}_{}",
        &uuid::Uuid::new_v4().simple().to_string()[..20]
    );
    let password = "correct horse battery";
    let household = Household::new(HouseholdName::new("Chez nous").unwrap());
    SqlxHouseholdRepository::new(pool.clone())
        .create(&household)
        .await
        .unwrap();
    let hash = Argon2Hasher::new()
        .hash(&Password::new(password).unwrap())
        .unwrap();
    SqlxUserRepository::new(pool.clone())
        .create(&User::new(
            household.id,
            Username::new(username.clone()).unwrap(),
            hash,
        ))
        .await
        .unwrap();
    login(router, &username, password).await
}

/// Crée une recette via l'API et renvoie son identifiant.
async fn create_recipe(router: &Router, cookie: &str, title: &str) -> String {
    let body = serde_json::json!({
        "title": title,
        "ingredients": [{ "name": "courgette", "amount": 600.0, "unit": "g" }],
        "steps": ["Émincer."]
    });
    let response = router
        .clone()
        .oneshot(json_request("POST", "/recipes", cookie, &body))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    json_body(response).await["id"].as_str().unwrap().to_owned()
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
    let config = Config::from_env();
    let router = app(pool.clone(), store, &config);

    let cookie = sign_in(&router, &pool, "cook").await;
    let recipe_id = create_recipe(&router, &cookie, "Ratatouille").await;

    // Sans cookie : 401.
    let anon = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/meal-plan?from=2026-07-13&to=2026-07-19")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(anon.status(), StatusCode::UNAUTHORIZED);

    // Semaine vide au départ.
    let week = get_week(&router, &cookie, "2026-07-13", "2026-07-19").await;
    assert_eq!(week.as_array().unwrap().len(), 0);

    // Placement, puis relecture.
    let placed = router
        .clone()
        .oneshot(json_request(
            "PUT",
            "/meal-plan/2026-07-13/dinner",
            &cookie,
            &serde_json::json!({ "recipe_id": recipe_id }),
        ))
        .await
        .unwrap();
    assert_eq!(placed.status(), StatusCode::NO_CONTENT);

    let week = get_week(&router, &cookie, "2026-07-13", "2026-07-19").await;
    let slots = week.as_array().unwrap();
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0]["slot"], "dinner");
    assert_eq!(slots[0]["recipe_id"], recipe_id);

    // Replacer sur le même créneau remplace (upsert), sans doublon.
    let other_id = create_recipe(&router, &cookie, "Tarte aux pommes").await;
    router
        .clone()
        .oneshot(json_request(
            "PUT",
            "/meal-plan/2026-07-13/dinner",
            &cookie,
            &serde_json::json!({ "recipe_id": other_id }),
        ))
        .await
        .unwrap();
    let week = get_week(&router, &cookie, "2026-07-13", "2026-07-19").await;
    assert_eq!(week.as_array().unwrap().len(), 1);
    assert_eq!(week[0]["recipe_id"], other_id);

    // Le cœur du sujet : la recette d'un AUTRE foyer est refusée par la FK
    // composite — 404, jamais 500, et rien n'est planifié.
    let intruder = sign_in(&router, &pool, "voisin").await;
    let foreign_recipe = create_recipe(&router, &intruder, "Chez le voisin").await;
    let refused = router
        .clone()
        .oneshot(json_request(
            "PUT",
            "/meal-plan/2026-07-14/lunch",
            &cookie,
            &serde_json::json!({ "recipe_id": foreign_recipe }),
        ))
        .await
        .unwrap();
    assert_eq!(refused.status(), StatusCode::NOT_FOUND);

    // Une plage démesurée est refusée plutôt que de tout ramener.
    let too_wide = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/meal-plan?from=2026-01-01&to=9999-12-31")
                .header(COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
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
                .header(COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cleared.status(), StatusCode::NO_CONTENT);

    let again = router
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/meal-plan/2026-07-13/dinner")
                .header(COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(again.status(), StatusCode::NOT_FOUND);
}

/// Lit la semaine et renvoie le corps JSON.
async fn get_week(router: &Router, cookie: &str, from: &str, to: &str) -> Value {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/meal-plan?from={from}&to={to}"))
                .header(COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await
}

/// Se connecte et renvoie le cookie de session (`id=...`).
async fn login(router: &Router, username: &str, password: &str) -> String {
    let body = serde_json::json!({ "username": username, "password": password });
    let response = router
        .clone()
        .oneshot(json_request("POST", "/auth/login", "", &body))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response
        .headers()
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .map(|c| c.split(';').next().unwrap_or("").to_owned())
        .find(|c| c.starts_with("id="))
        .expect("cookie de session")
}

/// Construit une requête JSON, avec cookie optionnel (`""` pour aucun).
fn json_request(method: &str, uri: &str, cookie: &str, body: &Value) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(CONTENT_TYPE, "application/json");
    if !cookie.is_empty() {
        builder = builder.header(COOKIE, cookie);
    }
    builder.body(Body::from(body.to_string())).unwrap()
}
