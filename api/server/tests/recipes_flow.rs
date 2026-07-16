//! Test d'intégration du CRUD recettes contre un **Postgres réel** (via l'API
//! HTTP complète, cookie de session inclus). Marqué `#[ignore]` — nécessite
//! `DATABASE_URL`. Lancer :
//!
//! ```sh
//! docker compose up -d
//! DATABASE_URL=postgres://weekmeals:weekmeals@localhost:5432/weekmeals \
//!     cargo test -p server --test recipes_flow -- --ignored
//! ```

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

#[tokio::test]
#[ignore = "nécessite un Postgres (DATABASE_URL) — voir docstring"]
async fn recipe_crud_flow() {
    let pool = pool(&database_url()).unwrap();
    sqlx::migrate!("../migrations")
        .run(&pool)
        .await
        .expect("migrations");
    let store = init_session_store(&pool).await.expect("store de sessions");
    let config = Config::from_env();

    // Seed d'un compte, puis login pour obtenir le cookie de session.
    let username = format!("cook_{}", &uuid::Uuid::new_v4().simple().to_string()[..24]);
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

    let router = app(pool, store, &config);
    let cookie = login(&router, &username, password).await;

    // Sans cookie : 401.
    let anon = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/recipes")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(anon.status(), StatusCode::UNAUTHORIZED);

    // Création.
    let create_body = serde_json::json!({
        "title": "Ratatouille",
        "prep_time_min": 25,
        "cook_time_min": 45,
        "ingredients": [
            { "name": "courgette", "amount": 600.0, "unit": "g" },
            { "name": "gousse d'ail", "amount": 3.0, "unit": "piece" }
        ],
        "steps": ["Émincer l'oignon.", "Laisser mijoter."]
    });
    let created = router
        .clone()
        .oneshot(json_request("POST", "/recipes", &cookie, &create_body))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let created = json_body(created).await;
    let id = created["id"].as_str().unwrap().to_owned();
    assert_eq!(created["ingredients"].as_array().unwrap().len(), 2);

    // Recherche par titre.
    let found = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/recipes?search=rata")
                .header(COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(found.status(), StatusCode::OK);
    let list = json_body(found).await;
    assert_eq!(list.as_array().unwrap().len(), 1);

    // Détail.
    let detail = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/recipes/{id}"))
                .header(COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(detail.status(), StatusCode::OK);

    // Mise à jour.
    let update_body = serde_json::json!({
        "title": "Ratatouille express",
        "ingredients": [{ "name": "courgette", "amount": 400.0, "unit": "g" }],
        "steps": ["Tout mettre à mijoter."]
    });
    let updated = router
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/recipes/{id}"),
            &cookie,
            &update_body,
        ))
        .await
        .unwrap();
    assert_eq!(updated.status(), StatusCode::OK);
    let updated = json_body(updated).await;
    assert_eq!(updated["title"], "Ratatouille express");
    assert_eq!(updated["ingredients"].as_array().unwrap().len(), 1);

    // Suppression, puis 404.
    let deleted = router
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/recipes/{id}"))
                .header(COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);

    let gone = router
        .oneshot(
            Request::builder()
                .uri(format!("/recipes/{id}"))
                .header(COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(gone.status(), StatusCode::NOT_FOUND);
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
