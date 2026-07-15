//! Couche présentation de `auth` : routes Axum, DTO, session cookie et
//! extractor d'authentification.
//!
//! Sessions via `tower-sessions` (cookie `HttpOnly`, `SameSite`) — pas de JWT
//! (même origine, cf. ADR-0002). L'identité authentifiée est stockée en session
//! sous [`SESSION_KEY`] ; l'extractor [`AuthUser`] la relit et **scope** les
//! requêtes des autres domaines au foyer courant.

use std::sync::Arc;

use axum::extract::{FromRequestParts, State};
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use kernel::{HouseholdId, UserId};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;
use uuid::Uuid;

use crate::application::commands::{LoginCommand, LoginHandler, LoginResponse, LogoutHandler};
use crate::domain::password::PasswordHasher;
use crate::domain::UserRepository;

/// Clé de stockage de l'identité en session.
const SESSION_KEY: &str = "auth_user";

/// Identité sérialisée en session. Types « bruts » (`Uuid`, `String`) pour un
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

/// État injecté dans les routes d'auth : ports du domaine, via trait objects
/// pour découpler la présentation des implémentations concrètes.
#[derive(Clone)]
pub struct AuthState {
    /// Repository des utilisateurs.
    pub users: Arc<dyn UserRepository>,
    /// Service de hachage.
    pub hasher: Arc<dyn PasswordHasher>,
}

/// Corps de la requête de login.
#[derive(Debug, Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

/// Corps de la réponse exposant l'identité (login réussi et `/me`).
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

/// Sous-router des routes d'authentification, monté par le `server`.
pub fn router(state: AuthState) -> Router {
    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/me", get(me))
        .with_state(state)
}

/// `POST /auth/login` — établit la session sur identifiants valides.
async fn login(
    session: Session,
    State(state): State<AuthState>,
    Json(body): Json<LoginRequest>,
) -> impl IntoResponse {
    let handler = LoginHandler::new(state.users.as_ref(), state.hasher.as_ref());
    let response = handler
        .handle(LoginCommand {
            username: body.username,
            password: body.password,
        })
        .await;

    match response {
        LoginResponse::Success {
            user_id,
            household_id,
            username,
        } => {
            let session_user = SessionUser {
                user_id: user_id.as_uuid(),
                household_id: household_id.as_uuid(),
                username,
            };
            // Anti-fixation : on régénère l'identifiant de session au login.
            if session.cycle_id().await.is_err()
                || session
                    .insert(SESSION_KEY, session_user.clone())
                    .await
                    .is_err()
            {
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            }
            (StatusCode::OK, Json(UserView::from(session_user))).into_response()
        }
        LoginResponse::InvalidCredentials => StatusCode::UNAUTHORIZED.into_response(),
        LoginResponse::Unavailable => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `POST /auth/logout` — invalide la session.
async fn logout(session: Session) -> impl IntoResponse {
    let _ = LogoutHandler::new().handle(Default::default());
    match session.flush().await {
        Ok(()) => StatusCode::NO_CONTENT,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// `GET /auth/me` — identité courante, ou 401.
async fn me(user: AuthUser) -> impl IntoResponse {
    (StatusCode::OK, Json(UserView::from(user.0)))
}
