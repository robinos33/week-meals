//! Crate `server` — couche HTTP (Axum) et composition de l'application.
//!
//! Le binaire monte ici les sous-routers exposés par chaque domaine sous le
//! préfixe `/api`, derrière une couche de sessions cookie (`tower-sessions`,
//! store SQLite) et une couche CORS.
//!
//! Le préfixe `/api` n'est pas cosmétique : en prod le même processus sert
//! aussi le build du front (cf. ADR-0007), et plusieurs routes se
//! chevauchaient à la racine (`/recipes` côté SPA comme côté API). Tout ce qui
//! n'est ni `/health` ni `/api/**` retombe donc sur les fichiers statiques.
//!
//! [`app`] construit le routeur à partir d'un `SqlitePool` afin que les tests
//! d'intégration puissent l'assembler sans ouvrir de socket (pool paresseux).

use std::sync::Arc;

use axum::http::{HeaderValue, Method};
use axum::{routing::get, Json, Router};
use serde::Serialize;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::SqlitePool;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};
use tower_sessions::cookie::time::Duration;
use tower_sessions::cookie::SameSite;
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::SqliteStore;

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
use recipes::infrastructure::{
    HttpRecipeScraper, R2Config, R2PhotoStorage, SqlxRecipeRepository, VolumePhotoStorage,
};
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
    /// Dossier du build front à servir en statique (`WEB_DIST`). `None` en dev,
    /// où Vite s'en charge ; renseigné dans l'image Docker (cf. ADR-0007).
    pub web_dist: Option<String>,
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
        let web_dist = std::env::var("WEB_DIST").ok().filter(|v| !v.is_empty());
        Self {
            web_origin,
            secure_cookie,
            same_site,
            auth_mode,
            rp_id,
            rp_origin,
            web_dist,
        }
    }
}

/// Migrations applicatives embarquées dans le binaire (`api/migrations`).
///
/// Jouées au démarrage : le déploiement mono-conteneur n'a pas d'étape
/// `sqlx-cli` séparée. La base étant un fichier attaché à une machine unique
/// (cf. ADR-0008), il n'y a par construction qu'un seul migrateur à la fois.
///
/// # Errors
/// Remonte l'erreur SQLx si une migration échoue ou si l'historique diverge.
pub async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("../migrations").run(pool).await
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

/// Ouvre un pool SQLite (connexions établies à la demande).
///
/// `database_url` est de la forme `sqlite:///data/weekmeals.db`. Le fichier est
/// créé s'il n'existe pas : au premier démarrage sur un volume Fly vierge, il
/// n'y a rien à provisionner à la main.
///
/// Les pragmas ne sont pas cosmétiques (cf. ADR-0008) :
/// - `foreign_keys` — **désactivées par défaut** dans SQLite ; sans elles tous
///   les `on delete cascade` du schéma seraient décoratifs ;
/// - `WAL` — lecteurs et écrivain cessent de se bloquer, et c'est le journal
///   que Litestream réplique ;
/// - `busy_timeout` — attendre plutôt que d'échouer sur `SQLITE_BUSY` ;
/// - `synchronous = NORMAL` — le compromis recommandé avec WAL.
///
/// # Errors
/// Remonte l'erreur SQLx si l'URL est invalide.
pub fn pool(database_url: &str) -> Result<SqlitePool, sqlx::Error> {
    let options = database_url
        .parse::<SqliteConnectOptions>()?
        .create_if_missing(true)
        .foreign_keys(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(std::time::Duration::from_secs(5))
        .synchronous(SqliteSynchronous::Normal);

    Ok(SqlitePoolOptions::new()
        .max_connections(5)
        .connect_lazy_with(options))
}

/// Construit le store de sessions et applique sa migration (table
/// `tower_sessions_session`). À appeler une fois au démarrage.
///
/// # Errors
/// Remonte l'erreur SQLx si la migration échoue.
pub async fn init_session_store(pool: &SqlitePool) -> Result<SqliteStore, sqlx::Error> {
    let store = SqliteStore::new(pool.clone());
    store.migrate().await?;
    Ok(store)
}

/// Construit le routeur complet de l'application Week Meals.
///
/// `session_store` est injecté (plutôt que reconstruit) pour que le démarrage
/// puisse d'abord jouer sa migration ; les tests peuvent passer un store non
/// migré tant qu'ils ne touchent pas la session.
pub fn app(pool: SqlitePool, session_store: SqliteStore, config: &Config) -> Router {
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
    let (photos, local_photos) = photo_storage_from_env();
    let recipe_state = RecipeState {
        recipes: Arc::new(SqlxRecipeRepository::new(pool.clone())),
        photos,
        local_photos,
        // Import par URL (#61) : garde SSRF, c'est le serveur qui fetch.
        scraper: Arc::new(HttpRecipeScraper::guarded()),
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

    // Les couches session/CORS ne portent que sur l'API : inutile d'ouvrir une
    // session pour servir un `.woff2`.
    let api = Router::new()
        .merge(presentation::router(auth_state))
        .merge(recipes::presentation::router(recipe_state))
        .merge(meal_plan::presentation::router(meal_plan_state))
        .merge(shopping_list::presentation::router(shopping_list_state))
        .layer(session_layer)
        .layer(cors);

    let router = Router::new()
        .route("/health", get(health))
        .nest("/api", api);

    match &config.web_dist {
        // Déploiement mono-app : tout le reste est le SPA. Les chemins inconnus
        // retombent sur `index.html` pour laisser le routeur client décider
        // (rechargement direct sur `/recipes/<id>`, par exemple).
        Some(dist) => router.fallback_service(
            ServeDir::new(dist).fallback(ServeFile::new(format!("{dist}/index.html"))),
        ),
        // Dev : le front est servi par Vite, l'API ne sert qu'elle-même.
        None => router,
    }
}

/// Choisit le backend de stockage photo depuis l'environnement (cf. ADR-0009),
/// dans l'ordre : **R2** si les `R2_*` sont là, sinon le **volume** si
/// `PHOTO_STORAGE_DIR` est défini, sinon rien (la présignature répond `503` et le
/// front garde le champ URL en repli).
///
/// Renvoie `(photos, local_photos)` : le premier sert la présignature (R2 ou
/// volume), le second n'est `Some` qu'en mode volume — il porte les routes de
/// dépôt/service des fichiers.
fn photo_storage_from_env() -> (
    Option<Arc<dyn PhotoStorage>>,
    Option<Arc<VolumePhotoStorage>>,
) {
    if let Some(r2) = r2_storage_from_env() {
        return (Some(r2), None);
    }
    if let Some(volume) = volume_storage_from_env() {
        // Même objet des deux côtés : la présignation (`photos`) et les routes
        // fichiers (`local_photos`) partagent la table des jetons en attente.
        return (Some(volume.clone()), Some(volume));
    }
    (None, None)
}

/// Stockage R2/MinIO depuis les `R2_*`, ou `None` si la config est incomplète.
fn r2_storage_from_env() -> Option<Arc<dyn PhotoStorage>> {
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
            tracing::warn!("stockage photo R2 désactivé : {error}");
            None
        }
    }
}

/// Stockage sur volume, activé par `PHOTO_STORAGE_DIR` (cf. ADR-0009). `None` si
/// la variable est absente, ou si le répertoire ne peut être créé.
///
/// `PHOTO_PUBLIC_BASE` fixe la base des URLs (défaut `/api/recipes/photos`,
/// relatif = même origine en prod ; à passer en absolu si le front est sur une
/// autre origine, p. ex. en dev).
fn volume_storage_from_env() -> Option<Arc<VolumePhotoStorage>> {
    let dir = std::env::var("PHOTO_STORAGE_DIR")
        .ok()
        .filter(|value| !value.is_empty())?;
    let url_base = std::env::var("PHOTO_PUBLIC_BASE")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "/api/recipes/photos".to_owned());

    match VolumePhotoStorage::new(dir, url_base, std::time::Duration::from_secs(900)) {
        Ok(storage) => Some(Arc::new(storage)),
        Err(error) => {
            tracing::warn!("stockage photo volume désactivé : {error}");
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
