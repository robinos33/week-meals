//! Test d'intégration du flux d'authentification par passkeys contre un
//! **Postgres réel** (cf. ADR-0006).
//!
//! Marqué `#[ignore]` : nécessite une base accessible via `DATABASE_URL`
//! (par défaut celle du `docker-compose`). Lancer :
//!
//! ```sh
//! docker compose up -d
//! DATABASE_URL=postgres://weekmeals:weekmeals@localhost:5432/weekmeals \
//!     cargo test -p server --test auth_flow -- --ignored
//! ```
//!
//! Les cérémonies WebAuthn exigent un authentificateur (Face ID, clé matérielle
//! ou virtuelle) : elles ne sont pas rejouables sans navigateur. Ce test couvre
//! donc tout ce qui l'est côté serveur : verrouillage par défaut (`/auth/me` →
//! 401), bascule ouverte/fermée de la fenêtre d'enrôlement, rejet d'un code
//! erroné, et démarrage des deux cérémonies (challenge renvoyé).

use auth::domain::pairing::{Argon2PairingHasher, PairingCode, PairingHasher};
use auth::domain::repository::OnboardingRepository;
use auth::infrastructure::SqlxOnboardingRepository;
use axum::body::Body;
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, StatusCode};
use chrono::{Duration, Utc};
use http_body_util::BodyExt;
use kernel::{HouseholdId, DEMO_HOUSEHOLD_ID};
use server::{app, init_session_store, pool, Config};
use tower::ServiceExt;

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://weekmeals:weekmeals@localhost:5432/weekmeals".to_owned())
}

fn get(uri: &str) -> Request<Body> {
    Request::builder().uri(uri).body(Body::empty()).unwrap()
}

fn post_json(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header(CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn json_body(response: axum::response::Response) -> serde_json::Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
#[ignore = "nécessite un Postgres (DATABASE_URL) — voir docstring"]
async fn passkey_enroll_and_login_ceremonies() {
    let pool = pool(&database_url()).unwrap();
    sqlx::migrate!("../migrations")
        .run(&pool)
        .await
        .expect("migrations socle");
    let store = init_session_store(&pool).await.expect("store de sessions");
    // Force le mode verrouillé quelle que soit la config d'environnement de CI.
    std::env::set_var("AUTH_MODE", "locked");
    let config = Config::from_env();
    let household = HouseholdId::from(DEMO_HOUSEHOLD_ID);

    // On part fenêtre fermée.
    let onboarding = SqlxOnboardingRepository::new(pool.clone());
    onboarding.close(household).await.unwrap();

    let router = app(pool.clone(), store, &config);

    // Verrouillé : pas de session ⇒ /auth/me → 401.
    let me = router.clone().oneshot(get("/auth/me")).await.unwrap();
    assert_eq!(me.status(), StatusCode::UNAUTHORIZED);

    // Fenêtre fermée ⇒ status open:false.
    let status = router
        .clone()
        .oneshot(get("/auth/enroll/status"))
        .await
        .unwrap();
    assert_eq!(status.status(), StatusCode::OK);
    assert_eq!(json_body(status).await["open"], serde_json::json!(false));

    // Ouverture d'une fenêtre avec un code connu.
    let code = PairingCode::generate();
    let hash = Argon2PairingHasher::new().hash(&code).unwrap();
    onboarding
        .open(household, Utc::now() + Duration::minutes(15), &hash, None)
        .await
        .unwrap();

    // Fenêtre ouverte ⇒ status open:true.
    let status = router
        .clone()
        .oneshot(get("/auth/enroll/status"))
        .await
        .unwrap();
    assert_eq!(json_body(status).await["open"], serde_json::json!(true));

    // Code erroné ⇒ 403.
    let bad = router
        .clone()
        .oneshot(post_json(
            "/auth/enroll/start",
            serde_json::json!({ "code": "AAAA-BBBB", "label": "iPhone de test" }),
        ))
        .await
        .unwrap();
    assert_eq!(bad.status(), StatusCode::FORBIDDEN);

    // Bon code ⇒ 200 + challenge d'enregistrement.
    let ok = router
        .clone()
        .oneshot(post_json(
            "/auth/enroll/start",
            serde_json::json!({ "code": code.formatted(), "label": "iPhone de test" }),
        ))
        .await
        .unwrap();
    assert_eq!(ok.status(), StatusCode::OK);
    assert!(json_body(ok).await.get("publicKey").is_some());

    // La cérémonie d'authentification découvrable démarre sans identifiant.
    let login = router
        .clone()
        .oneshot(post_json("/auth/login/start", serde_json::json!(null)))
        .await
        .unwrap();
    assert_eq!(login.status(), StatusCode::OK);
    assert!(json_body(login).await.get("publicKey").is_some());

    onboarding.close(household).await.unwrap();
}
