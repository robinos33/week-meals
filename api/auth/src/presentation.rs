//! Couche présentation de `auth` : routes Axum, DTO, session cookie, extractor
//! d'authentification et cérémonies WebAuthn (cf. ADR-0006).
//!
//! Sessions via `tower-sessions` (cookie `HttpOnly`, `SameSite`) — pas de JWT
//! (même origine). L'identité authentifiée est stockée en session sous
//! [`SESSION_KEY`] ; l'extractor [`AuthUser`] la relit et **scope** les requêtes
//! des autres domaines au foyer courant.
//!
//! Deux cérémonies :
//! - **Enrôlement** — un appareil inconnu, pendant la fenêtre d'onboarding et
//!   sur code d'appairage valide, enregistre une passkey (`/auth/enroll/*`).
//! - **Authentification découvrable** — le téléphone se présente sans identifiant
//!   (« Continuer avec Face ID »), le serveur l'identifie par le handle porté par
//!   la passkey (`/auth/login/*`).
//!
//! L'état de cérémonie (`PasskeyRegistration`, `DiscoverableAuthentication`) est
//! conservé **en session côté serveur**, jamais confié au client.

use std::sync::{Arc, OnceLock};

use axum::extract::{FromRequestParts, Path, State};
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use chrono::Utc;
use kernel::{DeviceId, HouseholdId, UserId, DEMO_HOUSEHOLD_ID};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use uuid::Uuid;
use webauthn_rs::prelude::{
    CreationChallengeResponse, DiscoverableAuthentication, DiscoverableKey, Passkey,
    PasskeyRegistration, PublicKeyCredential, RegisterPublicKeyCredential,
    RequestChallengeResponse, Url, Webauthn, WebauthnBuilder,
};

/// Ré-export du parseur d'URL de `webauthn-rs`, pour que le `server` déduise le
/// `rp_id` de l'origine sans dépendre directement d'`url`.
pub use webauthn_rs::prelude::Url as WebauthnUrl;

use crate::application::commands::{LogoutCommand, LogoutHandler};
use crate::domain::device::{Device, DeviceLabel};
use crate::domain::pairing::{PairingCode, PairingHasher};
use crate::domain::repository::{DeviceRepository, OnboardingRepository, UserRepository};
use crate::domain::user::{User, Username};

/// Clé de stockage de l'identité en session.
const SESSION_KEY: &str = "auth_user";
/// Clé de l'état de cérémonie d'enrôlement en session.
const ENROLL_KEY: &str = "enroll_ceremony";
/// Clé de l'état de cérémonie d'authentification en session.
const LOGIN_KEY: &str = "login_ceremony";

/// Utilisateur de démonstration servant d'identité en mode public. N'est pas
/// persisté (les recettes ne référencent que le foyer) — un UUID fixe suffit.
const DEMO_USER_ID: Uuid = Uuid::from_u128(0x0000_0000_0000_0000_0000_0000_0000_0002);

/// Mode d'authentification (cf. ADR-0006). Remonté dans la config du `server`
/// puis injecté ici une fois au démarrage via [`init_auth_mode`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMode {
    /// Dev : aucune identification, tout est scopé au foyer de démo.
    Disabled,
    /// Prod : seuls les appareils enrôlés passent.
    Locked,
}

/// Mode d'auth effectif du process. `Locked` par défaut (on échoue fermé) tant
/// qu'il n'a pas été explicitement initialisé.
static AUTH_MODE: OnceLock<AuthMode> = OnceLock::new();

/// Fixe le mode d'authentification, une fois, au démarrage. Les appels
/// ultérieurs sont ignorés (le premier gagne).
pub fn init_auth_mode(mode: AuthMode) {
    let _ = AUTH_MODE.set(mode);
}

/// Mode d'auth courant, `Locked` par défaut (fail-closed).
fn auth_mode() -> AuthMode {
    AUTH_MODE.get().copied().unwrap_or(AuthMode::Locked)
}

/// Construit l'instance `Webauthn` (Relying Party) à partir de l'identifiant de
/// domaine (`rp_id`) et de l'origine du front (`rp_origin`).
///
/// # Errors
/// Renvoie un message si l'origine n'est pas une URL valide ou si la
/// configuration WebAuthn est incohérente (rp_id absent de l'origine…).
pub fn build_webauthn(rp_id: &str, rp_origin: &str) -> Result<Webauthn, String> {
    let origin = Url::parse(rp_origin)
        .map_err(|e| format!("WEBAUTHN_RP_ORIGIN « {rp_origin} » invalide : {e}"))?;
    let builder = WebauthnBuilder::new(rp_id, &origin)
        .map_err(|e| format!("configuration WebAuthn invalide : {e}"))?
        .rp_name("Week Meals");
    builder
        .build()
        .map_err(|e| format!("construction WebAuthn : {e}"))
}

/// Identité sérialisée en session. Types « bruts » (`Uuid`, `String`) pour une
/// (dé)sérialisation stable indépendante des newtypes du domaine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionUser {
    /// Identifiant de l'utilisateur.
    pub user_id: Uuid,
    /// Foyer de l'utilisateur (scope des données).
    pub household_id: Uuid,
    /// Pseudo.
    pub username: String,
}

/// Utilisateur authentifié, extrait de la session. Utilisé par toutes les
/// routes protégées (y compris des autres domaines) pour scoper au foyer.
///
/// Rejette la requête en `401 Unauthorized` si aucune session valide.
#[derive(Debug, Clone)]
pub struct AuthUser(pub SessionUser);

impl AuthUser {
    /// Foyer de l'utilisateur (scope de toutes ses données).
    #[must_use]
    pub fn household_id(&self) -> HouseholdId {
        HouseholdId::from(self.0.household_id)
    }

    /// Identifiant de l'utilisateur.
    #[must_use]
    pub fn user_id(&self) -> UserId {
        UserId::from(self.0.user_id)
    }
}

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // Mode public : on court-circuite la session et on scope au foyer de démo.
        if auth_mode() == AuthMode::Disabled {
            return Ok(AuthUser(SessionUser {
                user_id: DEMO_USER_ID,
                household_id: DEMO_HOUSEHOLD_ID,
                username: "démo".to_owned(),
            }));
        }
        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(|_| StatusCode::UNAUTHORIZED)?;
        let user = session
            .get::<SessionUser>(SESSION_KEY)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        user.map(AuthUser).ok_or(StatusCode::UNAUTHORIZED)
    }
}

/// État injecté dans les routes d'auth : instance WebAuthn, ports du domaine et
/// le foyer courant (l'app est mono-foyer côté enrôlement).
#[derive(Clone)]
pub struct AuthState {
    /// Relying Party WebAuthn.
    pub webauthn: Arc<Webauthn>,
    /// Repository des utilisateurs.
    pub users: Arc<dyn UserRepository>,
    /// Repository des appareils enrôlés.
    pub devices: Arc<dyn DeviceRepository>,
    /// Pilotage de la fenêtre d'enrôlement.
    pub onboarding: Arc<dyn OnboardingRepository>,
    /// Hacheur du code d'appairage.
    pub hasher: Arc<dyn PairingHasher>,
    /// Foyer cible de l'enrôlement et des nouveaux utilisateurs.
    pub household_id: HouseholdId,
}

/// Corps de la réponse exposant l'identité (`/auth/me` et fins de cérémonie).
#[derive(Debug, Serialize)]
struct UserView {
    user_id: Uuid,
    household_id: Uuid,
    username: String,
}

impl From<SessionUser> for UserView {
    fn from(session: SessionUser) -> Self {
        Self {
            user_id: session.user_id,
            household_id: session.household_id,
            username: session.username,
        }
    }
}

/// État de cérémonie d'enrôlement, conservé en session le temps de l'aller-retour.
#[derive(Debug, Serialize, Deserialize)]
struct EnrollCeremony {
    reg: PasskeyRegistration,
    user_id: Uuid,
    household_id: Uuid,
    username: String,
    label: String,
    is_new_user: bool,
}

/// Sous-router des routes d'authentification, monté par le `server`.
pub fn router(state: AuthState) -> Router {
    Router::new()
        .route("/auth/me", get(me))
        .route("/auth/logout", post(logout))
        .route("/auth/enroll/status", get(enroll_status))
        .route("/auth/enroll/start", post(enroll_start))
        .route("/auth/enroll/finish", post(enroll_finish))
        .route("/auth/login/start", post(login_start))
        .route("/auth/login/finish", post(login_finish))
        .route("/auth/devices", get(list_devices))
        .route("/auth/devices/{id}", delete(revoke_device))
        .with_state(state)
}

/// `GET /auth/me` — identité courante, ou 401.
async fn me(user: AuthUser) -> impl IntoResponse {
    (StatusCode::OK, Json(UserView::from(user.0)))
}

/// `POST /auth/logout` — invalide la session.
async fn logout(session: Session) -> impl IntoResponse {
    let _ = LogoutHandler::new().handle(LogoutCommand);
    match session.flush().await {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Réponse de `GET /auth/enroll/status`.
#[derive(Debug, Serialize)]
struct EnrollStatus {
    /// La fenêtre d'enrôlement est-elle ouverte ?
    open: bool,
}

/// `GET /auth/enroll/status` — la fenêtre d'enrôlement est-elle ouverte ? Permet
/// au front de choisir entre l'écran d'enrôlement et l'écran « verrouillé ».
async fn enroll_status(State(state): State<AuthState>) -> impl IntoResponse {
    let open = matches!(
        state.onboarding.get(state.household_id).await,
        Ok(Some(window)) if window.is_open(Utc::now())
    );
    (StatusCode::OK, Json(EnrollStatus { open }))
}

/// Corps de `POST /auth/enroll/start`.
#[derive(Debug, Deserialize)]
struct EnrollStartRequest {
    /// Code d'appairage saisi.
    code: String,
    /// Libellé de l'appareil (« iPhone de Robin »).
    label: String,
}

/// `POST /auth/enroll/start` — vérifie fenêtre + code, démarre la cérémonie
/// d'enregistrement et renvoie le challenge. Un code erroné compte comme une
/// tentative (la fenêtre se referme au bout de cinq).
async fn enroll_start(
    session: Session,
    State(state): State<AuthState>,
    Json(body): Json<EnrollStartRequest>,
) -> Result<Json<CreationChallengeResponse>, StatusCode> {
    let Ok(label) = DeviceLabel::new(&body.label) else {
        return Err(StatusCode::BAD_REQUEST);
    };

    // Fenêtre ouverte ?
    let window = match state.onboarding.get(state.household_id).await {
        Ok(Some(window)) if window.is_open(Utc::now()) => window,
        Ok(_) => return Err(StatusCode::FORBIDDEN),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    // Code d'appairage valide ? Toute erreur (format, hash) compte comme un échec.
    let code_ok = PairingCode::parse(&body.code)
        .map(|code| state.hasher.verify(&code, &window.code_hash))
        .transpose()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .unwrap_or(false);
    if !code_ok {
        // Le compteur est la protection anti-force-brute : s'il ne s'incrémente
        // pas, elle disparaît. On refuse plutôt que de laisser passer en silence.
        match state.onboarding.record_failure(state.household_id).await {
            Ok(attempts) => tracing::warn!("code d'appairage refusé ({attempts} tentative(s))"),
            Err(error) => {
                tracing::error!("comptage des tentatives d'appairage : {error}");
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }
        }
        return Err(StatusCode::FORBIDDEN);
    }

    // Cible : utilisateur existant (`--for`) ou nouvel utilisateur à la fin.
    let (user_id, username, is_new_user) = match window.target_user {
        Some(uid) => match state.users.find(uid).await {
            Ok(Some(user)) => (user.id.as_uuid(), user.username.as_str().to_owned(), false),
            Ok(None) => return Err(StatusCode::FORBIDDEN),
            Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
        },
        None => (Uuid::new_v4(), username_from_label(&label), true),
    };

    let (challenge, reg) = state
        .webauthn
        .start_passkey_registration(user_id, &username, &username, None)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let ceremony = EnrollCeremony {
        reg,
        user_id,
        household_id: state.household_id.as_uuid(),
        username,
        label: label.as_str().to_owned(),
        is_new_user,
    };
    if session.insert(ENROLL_KEY, ceremony).await.is_err() {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    Ok(Json(challenge))
}

/// `POST /auth/enroll/finish` — valide l'attestation, enrôle l'appareil (crée
/// l'utilisateur si besoin) et ouvre directement la session.
async fn enroll_finish(
    session: Session,
    State(state): State<AuthState>,
    Json(credential): Json<RegisterPublicKeyCredential>,
) -> Result<Json<UserView>, StatusCode> {
    let ceremony = match session.get::<EnrollCeremony>(ENROLL_KEY).await {
        Ok(Some(ceremony)) => ceremony,
        Ok(None) => return Err(StatusCode::BAD_REQUEST),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    // La fenêtre est revérifiée ici : `enroll_start` peut dater, et rien
    // n'empêche une cérémonie entamée avant la fermeture d'aboutir après.
    match state.onboarding.get(state.household_id).await {
        Ok(Some(window)) if window.is_open(Utc::now()) => {}
        Ok(_) => return Err(StatusCode::FORBIDDEN),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    }

    let passkey = state
        .webauthn
        .finish_passkey_registration(&credential, &ceremony.reg)
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let passkey_json =
        serde_json::to_string(&passkey).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let user_id = UserId::from(ceremony.user_id);
    let household_id = HouseholdId::from(ceremony.household_id);

    // Nouvel utilisateur : on le crée avant l'appareil (contrainte de clé étrangère).
    if ceremony.is_new_user {
        let Ok(username) = Username::new(&ceremony.username) else {
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        };
        let user = User::from_parts(user_id, household_id, username);
        if state.users.create(&user).await.is_err() {
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }

    let Ok(label) = DeviceLabel::new(&ceremony.label) else {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    };
    let device = Device {
        id: DeviceId::new(),
        user_id,
        credential_id: passkey.cred_id().as_ref().to_vec(),
        passkey_json,
        label,
        // Les drapeaux de sauvegarde ne sont connus qu'à l'authentification :
        // on les renseignera au premier login (cf. `update_after_auth`).
        backup_eligible: false,
        backup_state: false,
        created_at: Utc::now(),
        last_seen_at: None,
    };
    if state.devices.create(&device).await.is_err() {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    // Code d'appairage à usage unique (ADR-0006) : la fenêtre se referme dès
    // qu'un appareil s'est enrôlé. Enrôler un second téléphone demande une
    // nouvelle `weekmeals device open-window`, donc un nouveau code.
    if let Err(error) = state.onboarding.close(state.household_id).await {
        // L'appareil est enrôlé ; échouer ici laisserait la fenêtre ouverte
        // sans que l'utilisateur puisse rien y faire — on trace et on refuse,
        // le CLI reste le recours (`device close-window`).
        tracing::error!("fermeture de la fenêtre d'enrôlement : {error}");
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    let _ = session.remove::<EnrollCeremony>(ENROLL_KEY).await;
    let session_user = SessionUser {
        user_id: ceremony.user_id,
        household_id: ceremony.household_id,
        username: ceremony.username,
    };
    establish_session(&session, &session_user).await?;
    Ok(Json(UserView::from(session_user)))
}

/// `POST /auth/login/start` — démarre une authentification découvrable (aucun
/// identifiant : le téléphone présentera la passkey de son choix).
///
/// `start_discoverable_authentication` positionne `mediation: "conditional"`
/// dans sa réponse (à côté de `publicKey`, pas dedans). Ce mode-là est celui de
/// l'**autofill** : il n'affiche aucune modale et attend qu'un champ
/// `autocomplete="username webauthn"` prenne le focus. Notre écran déclenche la
/// cérémonie sur un clic de bouton — le client ne transmet donc que `publicKey`
/// et laisse tomber `mediation` (cf. `web/src/api/auth.ts`). À reconsidérer si
/// l'on veut un jour la vraie UI conditionnelle.
async fn login_start(
    session: Session,
    State(state): State<AuthState>,
) -> Result<Json<RequestChallengeResponse>, StatusCode> {
    let (challenge, auth_state) = state
        .webauthn
        .start_discoverable_authentication()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if session.insert(LOGIN_KEY, auth_state).await.is_err() {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    Ok(Json(challenge))
}

/// `POST /auth/login/finish` — identifie l'utilisateur par le handle de la
/// passkey, valide l'assertion, met à jour le compteur et ouvre la session.
async fn login_finish(
    session: Session,
    State(state): State<AuthState>,
    Json(credential): Json<PublicKeyCredential>,
) -> Result<Json<UserView>, StatusCode> {
    let auth_state = match session.get::<DiscoverableAuthentication>(LOGIN_KEY).await {
        Ok(Some(state)) => state,
        Ok(None) => return Err(StatusCode::UNAUTHORIZED),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };

    // Le handle porté par la passkey identifie l'utilisateur sans saisie.
    let (user_uuid, _) = state
        .webauthn
        .identify_discoverable_authentication(&credential)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    let user_id = UserId::from(user_uuid);

    // Passkeys candidates de cet utilisateur.
    let devices = state
        .devices
        .list_by_user(user_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let passkeys: Vec<Passkey> = devices
        .iter()
        .filter_map(|d| serde_json::from_str::<Passkey>(&d.passkey_json).ok())
        .collect();
    if passkeys.is_empty() {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let discoverable: Vec<DiscoverableKey> = passkeys.iter().map(DiscoverableKey::from).collect();

    let result = state
        .webauthn
        .finish_discoverable_authentication(&credential, auth_state, &discoverable)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Met à jour le compteur de signature de la passkey concernée.
    let cred_id = result.cred_id().as_ref();
    if let Some(mut passkey) = passkeys
        .into_iter()
        .find(|p| p.cred_id().as_ref() == cred_id)
    {
        passkey.update_credential(&result);
        if let Ok(json) = serde_json::to_string(&passkey) {
            let _ = state
                .devices
                .update_after_auth(
                    cred_id,
                    &json,
                    result.backup_eligible(),
                    result.backup_state(),
                    Utc::now(),
                )
                .await;
        }
    }

    let user = match state.users.find(user_id).await {
        Ok(Some(user)) => user,
        Ok(None) => return Err(StatusCode::UNAUTHORIZED),
        Err(_) => return Err(StatusCode::INTERNAL_SERVER_ERROR),
    };
    let _ = session
        .remove::<DiscoverableAuthentication>(LOGIN_KEY)
        .await;
    let session_user = SessionUser {
        user_id: user.id.as_uuid(),
        household_id: user.household_id.as_uuid(),
        username: user.username.as_str().to_owned(),
    };
    establish_session(&session, &session_user).await?;
    Ok(Json(UserView::from(session_user)))
}

/// Vue d'un appareil enrôlé pour la carte Appareils des réglages.
#[derive(Debug, Serialize)]
struct DeviceView {
    id: Uuid,
    label: String,
    backup_state: bool,
    created_at: String,
    last_seen_at: Option<String>,
}

/// `GET /auth/devices` — liste les appareils enrôlés du foyer.
async fn list_devices(
    user: AuthUser,
    State(state): State<AuthState>,
) -> Result<Json<Vec<DeviceView>>, StatusCode> {
    let devices = state
        .devices
        .list_by_household(user.household_id())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let view = devices
        .into_iter()
        .map(|d| DeviceView {
            id: d.id.as_uuid(),
            label: d.label.as_str().to_owned(),
            backup_state: d.backup_state,
            created_at: d.created_at.to_rfc3339(),
            last_seen_at: d.last_seen_at.map(|t| t.to_rfc3339()),
        })
        .collect();
    Ok(Json(view))
}

/// `DELETE /auth/devices/{id}` — révoque un appareil du foyer courant.
///
/// Refuse de révoquer le **dernier** appareil : sans passkey restante, plus
/// personne ne peut entrer, et le seul recours serait un accès shell au serveur.
async fn revoke_device(
    user: AuthUser,
    State(state): State<AuthState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let remaining = match state.devices.list_by_household(user.household_id()).await {
        Ok(devices) => devices.len(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR,
    };
    if remaining <= 1 {
        return StatusCode::CONFLICT;
    }
    match state
        .devices
        .revoke(DeviceId::from(id), user.household_id())
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT,
        Ok(false) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Établit la session (anti-fixation : régénère l'identifiant) puis y insère
/// l'identité authentifiée.
async fn establish_session(session: &Session, user: &SessionUser) -> Result<(), StatusCode> {
    if session.cycle_id().await.is_err() || session.insert(SESSION_KEY, user).await.is_err() {
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    Ok(())
}

/// Dérive un pseudo (≤ 32 caractères) depuis le libellé d'appareil, pour un
/// nouvel utilisateur créé à l'enrôlement.
fn username_from_label(label: &DeviceLabel) -> String {
    let trimmed: String = label.as_str().chars().take(32).collect();
    trimmed.trim().to_owned()
}
