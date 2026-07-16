//! Couche présentation de `meal-plan` : routes Axum et DTO. Protégées par
//! l'extractor [`AuthUser`] (auth) et **scopées au foyer**.
//!
//! | Méthode | Route                     | Use case                 |
//! |---------|---------------------------|--------------------------|
//! | GET     | `/meal-plan?from&to`      | lire la semaine          |
//! | PUT     | `/meal-plan/{date}/{slot}`| placer une recette       |
//! | DELETE  | `/meal-plan/{date}/{slot}`| vider le créneau         |
//!
//! `date` est au format ISO `YYYY-MM-DD`, `slot` vaut `lunch` ou `dinner`. La
//! semaine renvoie les créneaux occupés (recette par identifiant) ; le front
//! résout le détail des recettes depuis sa grille déjà chargée.

use std::sync::Arc;

use auth::presentation::AuthUser;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use chrono::NaiveDate;
use kernel::RecipeId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::application::commands::{
    ClearMealCommand, ClearMealHandler, ClearMealResponse, PlaceMealCommand, PlaceMealHandler,
    PlaceMealResponse,
};
use crate::application::queries::{GetWeekHandler, GetWeekQuery, GetWeekResponse, MAX_RANGE_DAYS};
use crate::domain::{MealPlanRepository, PlannedMeal, Slot};

/// État injecté dans les routes du calendrier.
#[derive(Clone)]
pub struct MealPlanState {
    /// Repository du calendrier.
    pub plan: Arc<dyn MealPlanRepository>,
}

/// Sous-router du calendrier, monté par le `server`.
pub fn router(state: MealPlanState) -> Router {
    Router::new()
        .route("/meal-plan", get(week))
        .route(
            "/meal-plan/{date}/{slot}",
            axum::routing::put(place).delete(clear),
        )
        .with_state(state)
}

// --- DTO ------------------------------------------------------------------

/// Paramètres de la lecture de semaine (bornes incluses).
#[derive(Debug, Deserialize)]
struct WeekParams {
    from: NaiveDate,
    to: NaiveDate,
}

/// Corps du placement d'une recette sur un créneau.
#[derive(Debug, Deserialize)]
struct PlaceBody {
    recipe_id: Uuid,
}

/// Case du calendrier exposée en réponse.
#[derive(Debug, Serialize)]
struct PlannedMealView {
    date: NaiveDate,
    slot: &'static str,
    recipe_id: Uuid,
}

impl From<PlannedMeal> for PlannedMealView {
    fn from(meal: PlannedMeal) -> Self {
        Self {
            date: meal.date,
            slot: meal.slot.as_str(),
            recipe_id: meal.recipe_id.as_uuid(),
        }
    }
}

/// Interprète le créneau du chemin, ou 404 si inconnu.
fn parse_slot(raw: &str) -> Result<Slot, StatusCode> {
    Slot::parse(raw).ok_or(StatusCode::NOT_FOUND)
}

// --- Handlers -------------------------------------------------------------

/// `GET /meal-plan?from&to` — lit la semaine.
async fn week(
    user: AuthUser,
    State(state): State<MealPlanState>,
    Query(params): Query<WeekParams>,
) -> impl IntoResponse {
    let response = GetWeekHandler::new(state.plan.as_ref())
        .handle(GetWeekQuery {
            household_id: user.household_id(),
            start: params.from,
            end: params.to,
        })
        .await;
    match response {
        GetWeekResponse::Week(meals) => {
            let views: Vec<PlannedMealView> = meals.into_iter().map(Into::into).collect();
            (StatusCode::OK, Json(views)).into_response()
        }
        GetWeekResponse::RangeTooWide => (
            StatusCode::UNPROCESSABLE_ENTITY,
            format!("la plage demandée dépasse {MAX_RANGE_DAYS} jours"),
        )
            .into_response(),
        GetWeekResponse::Unavailable => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `PUT /meal-plan/{date}/{slot}` — place une recette sur le créneau.
async fn place(
    user: AuthUser,
    State(state): State<MealPlanState>,
    Path((date, slot)): Path<(NaiveDate, String)>,
    Json(body): Json<PlaceBody>,
) -> impl IntoResponse {
    let slot = match parse_slot(&slot) {
        Ok(slot) => slot,
        Err(status) => return status.into_response(),
    };
    let response = PlaceMealHandler::new(state.plan.as_ref())
        .handle(PlaceMealCommand {
            household_id: user.household_id(),
            date,
            slot,
            recipe_id: RecipeId::from(body.recipe_id),
        })
        .await;
    match response {
        PlaceMealResponse::Placed => StatusCode::NO_CONTENT.into_response(),
        PlaceMealResponse::RecipeNotFound => StatusCode::NOT_FOUND.into_response(),
        PlaceMealResponse::Unavailable => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `DELETE /meal-plan/{date}/{slot}` — vide le créneau.
async fn clear(
    user: AuthUser,
    State(state): State<MealPlanState>,
    Path((date, slot)): Path<(NaiveDate, String)>,
) -> impl IntoResponse {
    let slot = match parse_slot(&slot) {
        Ok(slot) => slot,
        Err(status) => return status.into_response(),
    };
    let response = ClearMealHandler::new(state.plan.as_ref())
        .handle(ClearMealCommand {
            household_id: user.household_id(),
            date,
            slot,
        })
        .await;
    match response {
        ClearMealResponse::Cleared => StatusCode::NO_CONTENT.into_response(),
        ClearMealResponse::NotFound => StatusCode::NOT_FOUND.into_response(),
        ClearMealResponse::Unavailable => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
