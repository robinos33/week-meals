//! Crate `server` — couche HTTP (Axum) et composition de l'application.
//!
//! Le binaire monte ici les sous-routers exposés par chaque domaine, derrière
//! une couche de sessions cookie (`tower-sessions`, store Postgres) et une
//! couche CORS pour le front (origine distincte en prod : Pages ↔ Scaleway).
//!
//! [`app`] construit le routeur à partir d'un `PgPool` afin que les tests
//! d'intégration puissent l'assembler sans ouvrir de socket (pool paresseux).

use std::sync::Arc;

use axum::http::{HeaderValue, Method};
use axum::{routing::get, Json, Router};
use serde::Serialize;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tower_http::cors::CorsLayer;
use tower_sessions::cookie::time::Duration;
use tower_sessions::cookie::SameSite;
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::PostgresStore;

use auth::domain::pairing::Argon2PairingHasher;
use auth::infrastructure::{
    SqlxDeviceRepository, SqlxHouseholdRepository, SqlxOnboardingRepository, SqlxUserRepository,
};
use auth::presentation::{self, AuthMode, AuthState};
use kernel::HouseholdId;
use kernel::DEMO_HOUSEHOLD_ID;
use meal_plan::infrastructure::SqlxMealPlanRepository;
use meal_plan::presentation::MealPlanState;
use recipes::domain::PhotoStorage;
use recipes::infrastructure::{R2Config, R2PhotoStorage, SqlxRecipeRepository};
use recipes::presentation::RecipeState;
use shopping_list::infrastructure::{
    SqlxCookedCounter, SqlxPlannedIngredients, SqlxReferenceRepository, SqlxShoppingListRepository,
};
use shopping_list::presentation::ShoppingListState;

/// Configuration HTTP lue depuis l'environnement.
#[derive(Debug, Clone)]
pub struct Config {
    /// Origine autorisée par CORS (front). Ex. `http://localhost:5173` en dev,
    /// l'URL Pages en prod. Les cookies de session étant impliqués, CORS est
    /// restreint à cette origine (pas de `*`).
    pub web_origin: String,
    /// `true` pour marquer le cookie `Secure` (HTTPS obligatoire) — activer en
    /// prod. Faux en dev HTTP local.
    pub secure_cookie: bool,
    /// `SameSite` du cookie. `Lax` en dev (même site), `None` requis pour un
    /// front et une API sur des domaines distincts (prod) — impose `Secure`.
    pub same_site: SameSite,
    /// Mode d'authentification (cf. ADR-0006). `Locked` par défaut (fail-closed).
    pub auth_mode: AuthMode,
    /// `rp_id` WebAuthn : le domaine (sans schéma ni port) partagé par le front
    /// et l'API. `localhost` en dev.
    pub rp_id: String,
    /// Origine WebAuthn : l'URL complète du front (schéma + domaine + port).
    pub rp_origin: String,
}

impl Config {
    /// Lit la configuration depuis l'environnement, avec des valeurs par défaut
    /// adaptées au dev local.
    #[must_use]
    pub fn from_env() -> Self {
        let web_origin =
            std::env::var("WEB_ORIGIN").unwrap_or_else(|_| "http://localhost:5173".to_owned());
        let secure_cookie = env_flag("SESSION_SECURE");
        let same_site = match std::env::var("SESSION_SAME_SITE").as_deref() {
            Ok("none") => SameSite::None,
            Ok("strict") => SameSite::Strict,
            _ => SameSite::Lax,
        };
        // Rétrocompatibilité : l'ancien `AUTH_DISABLED=1` équivaut à `AUTH_MODE=disabled`.
        // Casse ignorée, comme `env_flag` : un `AUTH_MODE=DISABLED` qui retombe
        // silencieusement sur `Locked` est incompréhensible côté dev.
        let auth_mode = match std::env::var("AUTH_MODE")
            .map(|v| v.to_lowercase())
            .as_deref()
        {
            Ok("disabled") => AuthMode::Disabled,
            Ok("locked") => AuthMode::Locked,
            _ if env_flag("AUTH_DISABLED") => AuthMode::Disabled,
            _ => AuthMode::Locked,
        };
        // Le `rp_id` par défaut se déduit de l'origine du front (son hôte) ;
        // `localhost` en dernier recours (dev).
        let rp_origin = std::env::var("WEBAUTHN_RP_ORIGIN").unwrap_or_else(|_| web_origin.clone());
        let rp_id = std::env::var("WEBAUTHN_RP_ID")
            .ok()
            .or_else(|| host_of(&rp_origin))
            .unwrap_or_else(|| "localhost".to_owned());
        Self {
            web_origin,
            secure_cookie,
            same_site,
            auth_mode,
            rp_id,
            rp_origin,
        }
    }
}

/// Extrait l'hôte (sans schéma ni port) d'une URL, pour déduire le `rp_id`.
/// S'appuie sur le parseur d'`url`, déjà présent via `webauthn-rs`, plutôt que
/// sur un découpage de chaîne à la main.
fn host_of(url: &str) -> Option<String> {
    auth::presentation::WebauthnUrl::parse(url)
        .ok()?
        .host_str()
        .map(str::to_owned)
}

/// Lit un booléen d'environnement (`1`/`true`/`yes`, insensible à la casse).
fn env_flag(key: &str) -> bool {
    matches!(
        std::env::var(key).map(|v| v.to_lowercase()).as_deref(),
        Ok("1" | "true" | "yes")
    )
}

/// Ouvre un pool Postgres (connexions établies à la demande).
///
/// # Errors
/// Remonte l'erreur SQLx si l'URL est invalide.
pub fn pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(5)
        .connect_lazy(database_url)
}

/// Construit le store de sessions et applique sa migration (table
/// `tower_sessions.session`). À appeler une fois au démarrage.
///
/// # Errors
/// Remonte l'erreur SQLx si la migration échoue.
pub async fn init_session_store(pool: &PgPool) -> Result<PostgresStore, sqlx::Error> {
    let store = PostgresStore::new(pool.clone());
    store.migrate().await?;
    Ok(store)
}

/// Construit le routeur complet de l'application Week Meals.
///
/// `session_store` est injecté (plutôt que reconstruit) pour que le démarrage
/// puisse d'abord jouer sa migration ; les tests peuvent passer un store non
/// migré tant qu'ils ne touchent pas la session.
pub fn app(pool: PgPool, session_store: PostgresStore, config: &Config) -> Router {
    let session_layer = SessionManagerLayer::new(session_store)
        .with_http_only(true)
        .with_same_site(config.same_site)
        .with_secure(config.secure_cookie)
        .with_expiry(Expiry::OnInactivity(Duration::days(30)));

    let cors = cors_layer(config);

    // Mode d'auth injecté une fois pour tout le process (lu par l'extractor
    // `AuthUser`, y compris depuis les autres domaines).
    presentation::init_auth_mode(config.auth_mode);

    let webauthn = presentation::build_webauthn(&config.rp_id, &config.rp_origin)
        .expect("configuration WebAuthn (WEBAUTHN_RP_ID / WEBAUTHN_RP_ORIGIN)");
    let auth_state = AuthState {
        webauthn: Arc::new(webauthn),
        users: Arc::new(SqlxUserRepository::new(pool.clone())),
        households: Arc::new(SqlxHouseholdRepository::new(pool.clone())),
        devices: Arc::new(SqlxDeviceRepository::new(pool.clone())),
        onboarding: Arc::new(SqlxOnboardingRepository::new(pool.clone())),
        hasher: Arc::new(Argon2PairingHasher::new()),
        household_id: HouseholdId::from(DEMO_HOUSEHOLD_ID),
    };
    let recipe_state = RecipeState {
        recipes: Arc::new(SqlxRecipeRepository::new(pool.clone())),
        photos: photo_storage_from_env(),
    };
    let meal_plan_state = MealPlanState {
        plan: Arc::new(SqlxMealPlanRepository::new(pool.clone())),
    };
    let shopping_list_state = ShoppingListState {
        items: Arc::new(SqlxShoppingListRepository::new(pool.clone())),
        references: Arc::new(SqlxReferenceRepository::new(pool.clone())),
        planned: Arc::new(SqlxPlannedIngredients::new(pool.clone())),
        cooked: Arc::new(SqlxCookedCounter::new(pool.clone())),
    };

    Router::new()
        .route("/health", get(health))
        .merge(presentation::router(auth_state))
        .merge(recipes::presentation::router(recipe_state))
        .merge(meal_plan::presentation::router(meal_plan_state))
        .merge(shopping_list::presentation::router(shopping_list_state))
        .layer(session_layer)
        .layer(cors)
}

/// Construit le stockage photo (R2/MinIO) depuis l'environnement, ou `None` si
/// la configuration est incomplète — dans ce cas la présignature répond `503`
/// et le front garde le champ URL en repli.
fn photo_storage_from_env() -> Option<Arc<dyn PhotoStorage>> {
    let var = |key: &str| std::env::var(key).ok().filter(|value| !value.is_empty());

    let config = R2Config {
        endpoint: var("R2_ENDPOINT")?,
        region: var("R2_REGION").unwrap_or_else(|| "auto".to_owned()),
        bucket: var("R2_BUCKET")?,
        access_key: var("R2_ACCESS_KEY_ID")?,
        secret_key: var("R2_SECRET_ACCESS_KEY")?,
        public_base_url: var("R2_PUBLIC_BASE_URL")?,
        // 15 min : le temps qu'un upload démarre, sans laisser traîner l'URL.
        expiry_secs: 900,
    };

    match R2PhotoStorage::new(config) {
        Ok(storage) => Some(Arc::new(storage)),
        Err(error) => {
            tracing::warn!("stockage photo désactivé : {error}");
            None
        }
    }
}

/// Couche CORS restreinte à l'origine du front, cookies autorisés.
fn cors_layer(config: &Config) -> CorsLayer {
    let mut cors = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([axum::http::header::CONTENT_TYPE])
        .allow_credentials(true);

    if let Ok(origin) = HeaderValue::from_str(&config.web_origin) {
        cors = cors.allow_origin(origin);
    }
    cors
}

/// Réponse du endpoint de santé.
#[derive(Serialize)]
struct Health {
    status: &'static str,
}

/// Endpoint de liveness : renvoie 200 tant que le process répond.
async fn health() -> Json<Health> {
    Json(Health { status: "ok" })
}
