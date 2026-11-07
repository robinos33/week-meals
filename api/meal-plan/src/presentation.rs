//! Couche présentation de `meal-plan` : routes Axum et DTO. Protégées par
//! l'extractor [`AuthUser`] (auth) et **scopées au foyer**.
//!
//! | Méthode | Route                     | Use case                 |
//! |---------|---------------------------|--------------------------|
//! | GET     | `/meal-plan?from&to`      | lire la semaine          |
//! | PUT     | `/meal-plan/{date}/{slot}`| placer un plat           |
//! | DELETE  | `/meal-plan/{date}/{slot}`| vider le créneau         |
//!
//! `date` est au format ISO `YYYY-MM-DD`, `slot` vaut `lunch` ou `dinner`. Le
//! corps du PUT porte **soit** `recipe_id` (recette du foyer), **soit** `label`
//! (repas libre : simple texte, rien n'entre dans la liste de courses) ; les
//! deux à la fois, ou aucun, donnent 422. La semaine renvoie les créneaux
//! occupés avec `recipe_id` ou `label` (l'autre à `null`) ; le front résout le
//! détail des recettes depuis sa grille déjà chargée.

use std::sync::Arc;

use auth::presentation::AuthUser;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use chrono::NaiveDate;
use kernel::{RecipeId, DEFAULT_SERVINGS};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::application::commands::{
    ClearMealCommand, ClearMealHandler, ClearMealResponse, PlaceMealCommand, PlaceMealHandler,
    PlaceMealResponse,
};
use crate::application::queries::{GetWeekHandler, GetWeekQuery, GetWeekResponse, MAX_RANGE_DAYS};
use crate::domain::{Dish, FreeLabel, MealPlanRepository, PlannedMeal, Slot, FREE_LABEL_MAX_CHARS};

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

/// Valeur par défaut du champ `servings` (créneau « pour 2 » à défaut).
fn default_servings() -> u32 {
    DEFAULT_SERVINGS
}

/// Corps du placement d'un plat sur un créneau : `recipe_id` **ou** `label`.
#[derive(Debug, Deserialize)]
struct PlaceBody {
    #[serde(default)]
    recipe_id: Option<Uuid>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default = "default_servings")]
    servings: u32,
}

/// Case du calendrier exposée en réponse.
#[derive(Debug, Serialize)]
struct PlannedMealView {
    date: NaiveDate,
    slot: &'static str,
    /// Recette placée, ou `null` pour un repas libre.
    recipe_id: Option<Uuid>,
    /// Libellé du repas libre, ou `null` pour une recette.
    label: Option<String>,
    servings: u32,
}

impl From<PlannedMeal> for PlannedMealView {
    fn from(meal: PlannedMeal) -> Self {
        let (recipe_id, label) = match meal.dish {
            Dish::Recipe(id) => (Some(id.as_uuid()), None),
            Dish::Free(label) => (None, Some(label.as_str().to_owned())),
        };
        Self {
            date: meal.date,
            slot: meal.slot.as_str(),
            recipe_id,
            label,
            servings: meal.servings,
        }
    }
}

/// Interprète le plat du corps : exactement un de `recipe_id` / `label`.
fn parse_dish(body: &PlaceBody) -> Result<Dish, (StatusCode, String)> {
    match (body.recipe_id, body.label.as_deref()) {
        (Some(id), None) => Ok(Dish::Recipe(RecipeId::from(id))),
        (None, Some(raw)) => FreeLabel::parse(raw).map(Dish::Free).ok_or_else(|| {
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                format!("le libellé doit faire entre 1 et {FREE_LABEL_MAX_CHARS} caractères"),
            )
        }),
        _ => Err((
            StatusCode::UNPROCESSABLE_ENTITY,
            "renseigner soit `recipe_id`, soit `label`".to_owned(),
        )),
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

/// `PUT /meal-plan/{date}/{slot}` — place un plat (recette ou repas libre).
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
    let dish = match parse_dish(&body) {
        Ok(dish) => dish,
        Err(rejection) => return rejection.into_response(),
    };
    let response = PlaceMealHandler::new(state.plan.as_ref())
        .handle(PlaceMealCommand {
            household_id: user.household_id(),
            date,
            slot,
            dish,
            servings: body.servings,
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
