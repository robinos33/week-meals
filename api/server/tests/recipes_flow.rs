//! Test d'intégration du CRUD recettes contre un **Postgres réel** (via l'API
//! HTTP complète). Marqué `#[ignore]` — nécessite `DATABASE_URL`. Lancer :
//!
//! ```sh
//! docker compose up -d
//! DATABASE_URL=postgres://weekmeals:weekmeals@localhost:5432/weekmeals \
//!     cargo test -p server --test recipes_flow -- --ignored
//! ```
//!
//! L'authentification par passkeys (ADR-0006) n'est pas rejouable sans
//! authentificateur : ce test s'exécute donc en **mode public** (`AUTH_MODE=
//! disabled`), où les requêtes sont scopées au foyer de démo sans session. Le
//! verrouillage lui-même est couvert par `auth_flow`. Un marqueur unique par
//! exécution garde le test isolé malgré le foyer partagé.

use axum::body::Body;
use axum::http::header::CONTENT_TYPE;
use axum::http::{Request, StatusCode};
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
    // Mode public : l'extractor scope au foyer de démo sans session.
    std::env::set_var("AUTH_MODE", "disabled");
    let config = Config::from_env();
    let router = app(pool, store, &config);

    // Marqueur unique : le foyer de démo est partagé entre exécutions.
    let marker = uuid::Uuid::new_v4().simple().to_string();
    let title = format!("Ratatouille {marker}");

    // Création.
    let create_body = serde_json::json!({
        "title": title,
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
        .oneshot(json_request("POST", "/recipes", &create_body))
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let created = json_body(created).await;
    let id = created["id"].as_str().unwrap().to_owned();
    assert_eq!(created["ingredients"].as_array().unwrap().len(), 2);

    // Recherche par le marqueur unique.
    let found = router
        .clone()
        .oneshot(get(&format!("/recipes?search={marker}")))
        .await
        .unwrap();
    assert_eq!(found.status(), StatusCode::OK);
    let list = json_body(found).await;
    assert_eq!(list.as_array().unwrap().len(), 1);

    // Détail.
    let detail = router
        .clone()
        .oneshot(get(&format!("/recipes/{id}")))
        .await
        .unwrap();
    assert_eq!(detail.status(), StatusCode::OK);

    // Mise à jour.
    let update_body = serde_json::json!({
        "title": format!("Ratatouille express {marker}"),
        "ingredients": [{ "name": "courgette", "amount": 400.0, "unit": "g" }],
        "steps": ["Tout mettre à mijoter."]
    });
    let updated = router
        .clone()
        .oneshot(json_request("PUT", &format!("/recipes/{id}"), &update_body))
        .await
        .unwrap();
    assert_eq!(updated.status(), StatusCode::OK);
    let updated = json_body(updated).await;
    assert_eq!(updated["title"], format!("Ratatouille express {marker}"));
    assert_eq!(updated["ingredients"].as_array().unwrap().len(), 1);

    // Suppression, puis 404.
    let deleted = router
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/recipes/{id}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);

    let gone = router
        .clone()
        .oneshot(get(&format!("/recipes/{id}")))
        .await
        .unwrap();
    assert_eq!(gone.status(), StatusCode::NOT_FOUND);

    // Import par URL (#61) : la route est montée et la garde SSRF refuse une
    // adresse interne (422), sans dépendre du réseau.
    let non_https = router
        .clone()
        .oneshot(json_request(
            "POST",
            "/recipes/scrape",
            &serde_json::json!({ "url": "http://example.com/rata" }),
        ))
        .await
        .unwrap();
    assert_eq!(non_https.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let loopback = router
        .oneshot(json_request(
            "POST",
            "/recipes/scrape",
            &serde_json::json!({ "url": "https://127.0.0.1/rata" }),
        ))
        .await
        .unwrap();
    assert_eq!(loopback.status(), StatusCode::UNPROCESSABLE_ENTITY);
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
