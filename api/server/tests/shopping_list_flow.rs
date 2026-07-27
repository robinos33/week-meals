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

/// Crée une recette « pour `servings` personnes » avec 600 g de courgette.
async fn create_recipe_with_servings(router: &Router, title: &str, servings: u32) -> String {
    let body = serde_json::json!({
        "title": title,
        "servings": servings,
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

/// Crée une recette avec les ingrédients donnés (`(nom, quantité, unité)`).
async fn create_recipe_with(
    router: &Router,
    title: &str,
    ingredients: &[(&str, f64, &str)],
) -> String {
    let ingredients: Vec<Value> = ingredients
        .iter()
        .map(|(name, amount, unit)| {
            serde_json::json!({ "name": name, "amount": amount, "unit": unit })
        })
        .collect();
    let body = serde_json::json!({
        "title": title,
        "ingredients": ingredients,
        "steps": ["Cuisiner."]
    });
    let response = router
        .clone()
        .oneshot(json_request("POST", "/api/recipes", &body))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    json_body(response).await["id"].as_str().unwrap().to_owned()
}

/// Charge le dictionnaire versionné dans la base de test — c'est ce que fait
/// le serveur à son démarrage.
async fn seed_dictionary(pool: &sqlx::SqlitePool) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/ingredients.yaml");
    shopping_list::infrastructure::seed::seed_from_file(pool, &path)
        .await
        .expect("seed du dictionnaire");
}

/// Lit le dictionnaire exposé au front.
async fn dictionary(router: &Router) -> Value {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/shopping-list/dictionary")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    json_body(response).await
}

/// Pose une recette sur un créneau du calendrier (2 personnes par défaut).
async fn plan(router: &Router, date: &str, slot: &str, recipe_id: &str) {
    plan_for(router, date, slot, recipe_id, 2).await;
}

/// Pose une recette sur un créneau pour un nombre de convives donné.
async fn plan_for(router: &Router, date: &str, slot: &str, recipe_id: &str, servings: u32) {
    let response = router
        .clone()
        .oneshot(json_request(
            "PUT",
            &format!("/api/meal-plan/{date}/{slot}"),
            &serde_json::json!({ "recipe_id": recipe_id, "servings": servings }),
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

/// Ajoute un article à la main et renvoie la réponse JSON de la ligne créée
/// ou fusionnée.
async fn add_item(router: &Router, name: &str, amount: f64, unit: &str) -> Value {
    let response = router
        .clone()
        .oneshot(json_request(
            "POST",
            "/api/shopping-list/items",
            &serde_json::json!({ "name": name, "amount": amount, "unit": unit }),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    json_body(response).await
}

/// Lit la liste de courses courante.
async fn list_items(router: &Router) -> Value {
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/shopping-list")
                .body(Body::empty())
                .unwrap(),
        )
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
async fn a_manually_added_item_lands_at_the_top() {
    let db = common::temp_database().await;
    let pool = db.pool.clone();
    let store = init_session_store(&pool).await.expect("store de sessions");
    std::env::set_var("AUTH_MODE", "disabled");
    let config = Config::from_env();
    let router = app(pool, store, &config);

    // Une liste générée occupe la liste ; l'ajout manuel doit passer devant.
    let recipe = create_recipe(&router, "Ratatouille").await;
    plan(&router, "2026-07-13", "dinner", &recipe).await;
    generate(&router, "2026-07-13", "2026-07-19").await;

    add_item(&router, "sel", 1.0, "piece").await;

    let items = list_items(&router).await;
    let items = items.as_array().unwrap();
    // « sel » (ajout manuel) en tête, devant la courgette générée.
    assert_eq!(items[0]["name"], "sel");
    assert_eq!(items[1]["name"], "courgette");
}

#[tokio::test]
async fn generating_scales_quantities_by_servings() {
    let db = common::temp_database().await;
    let pool = db.pool.clone();
    let store = init_session_store(&pool).await.expect("store de sessions");
    std::env::set_var("AUTH_MODE", "disabled");
    let config = Config::from_env();
    let router = app(pool, store, &config);

    // Recette « pour 2 » (600 g), planifiée pour 4 → quantités doublées.
    let for_two = create_recipe_with_servings(&router, "Rata pour 2", 2).await;
    plan_for(&router, "2026-07-13", "dinner", &for_two, 4).await;
    let items = generate(&router, "2026-07-13", "2026-07-19").await;
    let items = items.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], "courgette");
    assert_eq!(items[0]["amount"], 1200.0); // 600 × 4/2

    // Recette « pour 4 » (600 g), planifiée pour 2 → quantités divisées par 2.
    let for_four = create_recipe_with_servings(&router, "Rata pour 4", 4).await;
    plan_for(&router, "2026-07-14", "dinner", &for_four, 2).await;
    let items = generate(&router, "2026-07-14", "2026-07-14").await;
    let items = items.as_array().unwrap();
    assert_eq!(items[0]["amount"], 300.0); // 600 × 2/4
}

#[tokio::test]
async fn adding_an_existing_ingredient_sums_quantities() {
    let db = common::temp_database().await;
    let pool = db.pool.clone();
    let store = init_session_store(&pool).await.expect("store de sessions");
    std::env::set_var("AUTH_MODE", "disabled");
    let config = Config::from_env();
    let router = app(pool, store, &config);

    // Deux ajouts du même ingrédient : une seule ligne, quantités cumulées.
    add_item(&router, "courgette", 3.0, "piece").await;
    add_item(&router, "  Courgette ", 2.0, "piece").await; // casse/espaces ignorés

    let items = list_items(&router).await;
    let items = items.as_array().unwrap();
    assert_eq!(items.len(), 1, "pas de doublon");
    assert_eq!(items[0]["name"], "courgette");
    assert_eq!(items[0]["amount"], 5.0);

    // Somme inter-unités : 500 g + 0,5 kg → 1000 g dans l'unité de la ligne.
    add_item(&router, "farine", 500.0, "g").await;
    add_item(&router, "farine", 0.5, "kg").await;
    let items = list_items(&router).await;
    let items = items.as_array().unwrap();
    let farine = items.iter().find(|i| i["name"] == "farine").unwrap();
    assert_eq!(farine["amount"], 1000.0);
    assert_eq!(farine["unit"], "g");
}

#[tokio::test]
async fn manual_add_sums_onto_a_generated_line() {
    let db = common::temp_database().await;
    let pool = db.pool.clone();
    let store = init_session_store(&pool).await.expect("store de sessions");
    std::env::set_var("AUTH_MODE", "disabled");
    let config = Config::from_env();
    let router = app(pool, store, &config);

    // 600 g de courgette générés depuis le calendrier…
    let ratatouille = create_recipe(&router, "Ratatouille").await;
    plan(&router, "2026-07-13", "dinner", &ratatouille).await;
    generate(&router, "2026-07-13", "2026-07-19").await;

    // … + 300 g ajoutés à la main : une seule ligne (900 g), toujours générée.
    add_item(&router, "courgette", 300.0, "g").await;
    let items = list_items(&router).await;
    let items = items.as_array().unwrap();
    assert_eq!(items.len(), 1, "fusion sur la ligne générée");
    assert_eq!(items[0]["amount"], 900.0);
    assert_eq!(items[0]["unit"], "g");
    assert_eq!(items[0]["generated"], true);
}

#[tokio::test]
async fn the_dictionary_reconciles_recipe_wordings() {
    let db = common::temp_database().await;
    let pool = db.pool.clone();
    let store = init_session_store(&pool).await.expect("store de sessions");
    std::env::set_var("AUTH_MODE", "disabled");
    let config = Config::from_env();
    let router = app(pool.clone(), store, &config);
    seed_dictionary(&pool).await;

    // Deux recettes qui nomment différemment les mêmes produits.
    let gratin = create_recipe_with(
        &router,
        "Gratin",
        &[("Courgettes", 600.0, "g"), ("Oeufs", 2.0, "piece")],
    )
    .await;
    let poelee = create_recipe_with(
        &router,
        "Poêlée",
        &[("courgette jaune", 300.0, "g"), ("œuf", 1.0, "piece")],
    )
    .await;
    plan(&router, "2026-07-13", "dinner", &gratin).await;
    plan(&router, "2026-07-14", "dinner", &poelee).await;

    let items = generate(&router, "2026-07-13", "2026-07-19").await;
    let items = items.as_array().unwrap();
    assert_eq!(items.len(), 2, "une ligne par produit, pas par formulation");

    // 900 g de courgette → 4 pièces (250 g l'unité), sous le nom du dictionnaire.
    let courgette = items.iter().find(|i| i["name"] == "courgette").unwrap();
    assert_eq!(courgette["amount"], 4.0);
    assert_eq!(courgette["unit"], "piece");
    assert_eq!(courgette["category"], "legumes");

    // 2 + 1 œufs, sous le nom canonique malgré les deux graphies.
    let oeuf = items.iter().find(|i| i["name"] == "œuf").unwrap();
    assert_eq!(oeuf["amount"], 3.0);

    // Et l'ajout manuel d'une variante cumule sur la ligne existante.
    add_item(&router, "Courgettes", 2.0, "piece").await;
    let items = list_items(&router).await;
    let items = items.as_array().unwrap();
    assert_eq!(items.len(), 2, "toujours pas de doublon");
    let courgette = items.iter().find(|i| i["name"] == "courgette").unwrap();
    assert_eq!(courgette["amount"], 6.0);
}

#[tokio::test]
async fn the_dictionary_is_exposed_for_input_suggestions() {
    let db = common::temp_database().await;
    let pool = db.pool.clone();
    let store = init_session_store(&pool).await.expect("store de sessions");
    std::env::set_var("AUTH_MODE", "disabled");
    let config = Config::from_env();
    let router = app(pool.clone(), store, &config);

    // Base neuve : le dictionnaire est vide mais la route répond.
    assert_eq!(dictionary(&router).await.as_array().unwrap().len(), 0);

    seed_dictionary(&pool).await;
    let entries = dictionary(&router).await;
    let entries = entries.as_array().unwrap();
    assert!(entries.len() > 50, "le dictionnaire versionné est chargé");

    let courgette = entries.iter().find(|e| e["name"] == "courgette").unwrap();
    assert_eq!(courgette["category"], "legumes");
    assert_eq!(courgette["unit"], "piece");

    // Une entrée sans poids moyen garde son unité d'achat.
    let lait = entries.iter().find(|e| e["name"] == "lait").unwrap();
    assert_eq!(lait["unit"], "ml");
    // Les synonymes voyagent avec l'entrée : le front s'en sert pour retrouver
    // « œuf » à partir de « oeuf ».
    let oeuf = entries.iter().find(|e| e["name"] == "œuf").unwrap();
    assert!(oeuf["aliases"]
        .as_array()
        .unwrap()
        .iter()
        .any(|alias| alias == "oeuf"));
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
