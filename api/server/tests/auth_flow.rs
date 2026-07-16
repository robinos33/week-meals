//! Test d'intégration du flux d'authentification contre un **Postgres réel**.
//!
//! Marqué `#[ignore]` : nécessite une base accessible via `DATABASE_URL`
//! (par défaut celle du `docker-compose`). Lancer :
//!
//! ```sh
//! docker compose up -d
//! sqlx migrate run --source api/migrations   # ou laisser le test migrer
//! DATABASE_URL=postgres://weekmeals:weekmeals@localhost:5432/weekmeals \
//!     cargo test -p server --test auth_flow -- --ignored
//! ```
//!
//! Vérifie : login (401 sur mauvais mot de passe, 200 + cookie sur bon), accès
//! à `/auth/me` avec le cookie, puis logout et perte d'accès (401).

use auth::domain::household::{Household, HouseholdName};
use auth::domain::password::{Argon2Hasher, Password, PasswordHasher};
use auth::domain::user::{User, Username};
use auth::domain::{HouseholdRepository, UserRepository};
use auth::infrastructure::{SqlxHouseholdRepository, SqlxUserRepository};
use axum::body::Body;
use axum::http::header::{CONTENT_TYPE, COOKIE, SET_COOKIE};
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use server::{app, init_session_store, pool, Config};
use tower::ServiceExt;

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://weekmeals:weekmeals@localhost:5432/weekmeals".to_owned())
}

/// Extrait la valeur `name=value` du cookie de session depuis les en-têtes
/// `Set-Cookie` d'une réponse.
fn session_cookie(headers: &axum::http::HeaderMap) -> String {
    headers
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .map(|c| c.split(';').next().unwrap_or("").to_owned())
        .find(|c| c.starts_with("id="))
        .expect("cookie de session dans la réponse de login")
}

#[tokio::test]
#[ignore = "nécessite un Postgres (DATABASE_URL) — voir docstring"]
async fn login_me_logout_flow() {
    let pool = pool(&database_url()).unwrap();
    sqlx::migrate!("../migrations")
        .run(&pool)
        .await
        .expect("migrations socle");
    let store = init_session_store(&pool).await.expect("store de sessions");
    let config = Config::from_env();

    // Seed : un foyer + un utilisateur avec un mot de passe connu, pseudo unique.
    let username = format!("robin_{}", uuid::Uuid::new_v4().simple());
    let password = "correct horse battery";
    let household = Household::new(HouseholdName::new("Chez nous").unwrap());
    SqlxHouseholdRepository::new(pool.clone())
        .create(&household)
        .await
        .unwrap();
    let hash = Argon2Hasher::new()
        .hash(&Password::new(password).unwrap())
        .unwrap();
    let user = User::new(
        household.id,
        Username::new(username.clone()).unwrap(),
        hash,
    );
    SqlxUserRepository::new(pool.clone())
        .create(&user)
        .await
        .unwrap();

    let router = app(pool, store, &config);

    // Mauvais mot de passe → 401.
    let bad = router
        .clone()
        .oneshot(login_request(&username, "wrong password!!"))
        .await
        .unwrap();
    assert_eq!(bad.status(), StatusCode::UNAUTHORIZED);

    // Bon mot de passe → 200 + cookie de session.
    let ok = router
        .clone()
        .oneshot(login_request(&username, password))
        .await
        .unwrap();
    assert_eq!(ok.status(), StatusCode::OK);
    let cookie = session_cookie(ok.headers());
    let body = ok.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["username"], username);

    // /auth/me avec le cookie → 200.
    let me = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/me")
                .header(COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me.status(), StatusCode::OK);

    // Logout → 204.
    let out = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/logout")
                .header(COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(out.status(), StatusCode::NO_CONTENT);

    // /auth/me après logout (même cookie) → 401.
    let me_after = router
        .oneshot(
            Request::builder()
                .uri("/auth/me")
                .header(COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(me_after.status(), StatusCode::UNAUTHORIZED);
}

fn login_request(username: &str, password: &str) -> Request<Body> {
    let body = serde_json::json!({ "username": username, "password": password });
    Request::builder()
        .method("POST")
        .uri("/auth/login")
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}
