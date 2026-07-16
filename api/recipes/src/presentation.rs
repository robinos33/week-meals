//! Couche présentation de `recipes` : routes Axum et DTO. Toutes les routes
//! sont protégées par l'extractor [`AuthUser`] (auth) et **scopées au foyer**
//! de l'utilisateur connecté.
//!
//! | Méthode | Route            | Use case                    |
//! |---------|------------------|-----------------------------|
//! | GET     | `/recipes`       | liste / recherche (`?search`) |
//! | POST    | `/recipes`       | création                    |
//! | GET     | `/recipes/:id`   | détail                      |
//! | PUT     | `/recipes/:id`   | mise à jour                 |
//! | DELETE  | `/recipes/:id`   | suppression                 |

use std::sync::Arc;

use auth::presentation::AuthUser;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use kernel::{RecipeId, Unit};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::application::commands::{
    CreateRecipeCommand, CreateRecipeHandler, CreateRecipeResponse, DeleteRecipeCommand,
    DeleteRecipeHandler, DeleteRecipeResponse, RecipeFields, UpdateRecipeCommand,
    UpdateRecipeHandler, UpdateRecipeResponse,
};
use crate::application::queries::{
    GetRecipeHandler, GetRecipeQuery, GetRecipeResponse, ListRecipesHandler, ListRecipesQuery,
    ListRecipesResponse,
};
use crate::application::IngredientInput;
use crate::domain::{Recipe, RecipeRepository};

/// État injecté dans les routes recettes.
#[derive(Clone)]
pub struct RecipeState {
    /// Repository des recettes.
    pub recipes: Arc<dyn RecipeRepository>,
}

/// Sous-router des recettes, monté par le `server`.
pub fn router(state: RecipeState) -> Router {
    Router::new()
        .route("/recipes", get(list).post(create))
        .route("/recipes/{id}", get(detail).put(update).delete(delete))
        .with_state(state)
}

// --- DTO ------------------------------------------------------------------

/// Ingrédient dans le corps d'une requête.
#[derive(Debug, Deserialize)]
struct IngredientBody {
    name: String,
    amount: f64,
    unit: Unit,
}

impl From<IngredientBody> for IngredientInput {
    fn from(body: IngredientBody) -> Self {
        Self {
            name: body.name,
            amount: body.amount,
            unit: body.unit,
        }
    }
}

/// Corps de création / mise à jour d'une recette.
#[derive(Debug, Deserialize)]
struct RecipeBody {
    title: String,
    #[serde(default)]
    prep_time_min: Option<u32>,
    #[serde(default)]
    cook_time_min: Option<u32>,
    #[serde(default)]
    photo: Option<String>,
    #[serde(default)]
    ingredients: Vec<IngredientBody>,
    #[serde(default)]
    steps: Vec<String>,
}

impl From<RecipeBody> for RecipeFields {
    fn from(body: RecipeBody) -> Self {
        Self {
            title: body.title,
            prep_time_min: body.prep_time_min,
            cook_time_min: body.cook_time_min,
            photo: body.photo,
            ingredients: body.ingredients.into_iter().map(Into::into).collect(),
            steps: body.steps,
        }
    }
}

/// Ingrédient exposé en réponse.
#[derive(Debug, Serialize)]
struct IngredientView {
    name: String,
    amount: f64,
    unit: Unit,
}

/// Recette exposée en réponse.
#[derive(Debug, Serialize)]
struct RecipeView {
    id: Uuid,
    household_id: Uuid,
    title: String,
    photo: Option<String>,
    prep_time_min: Option<u32>,
    cook_time_min: Option<u32>,
    ingredients: Vec<IngredientView>,
    steps: Vec<String>,
}

impl From<Recipe> for RecipeView {
    fn from(recipe: Recipe) -> Self {
        Self {
            id: recipe.id.as_uuid(),
            household_id: recipe.household_id.as_uuid(),
            title: recipe.title,
            photo: recipe.photo,
            prep_time_min: recipe.prep_time_min,
            cook_time_min: recipe.cook_time_min,
            ingredients: recipe
                .ingredients
                .into_iter()
                .map(|i| IngredientView {
                    name: i.name,
                    amount: i.quantity.amount(),
                    unit: i.quantity.unit(),
                })
                .collect(),
            steps: recipe.steps,
        }
    }
}

/// Corps d'erreur uniforme.
#[derive(Debug, Serialize)]
struct ErrorBody {
    error: String,
}

fn invalid(message: String) -> axum::response::Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(ErrorBody { error: message }),
    )
        .into_response()
}

/// Paramètres de la liste : recherche optionnelle par titre.
#[derive(Debug, Deserialize)]
struct ListParams {
    #[serde(default)]
    search: Option<String>,
}

// --- Handlers -------------------------------------------------------------

/// `GET /recipes` — liste ou recherche.
async fn list(
    user: AuthUser,
    State(state): State<RecipeState>,
    Query(params): Query<ListParams>,
) -> impl IntoResponse {
    let response = ListRecipesHandler::new(state.recipes.as_ref())
        .handle(ListRecipesQuery {
            household_id: user.household_id(),
            search: params.search,
        })
        .await;
    match response {
        ListRecipesResponse::Listed(recipes) => {
            let views: Vec<RecipeView> = recipes.into_iter().map(RecipeView::from).collect();
            (StatusCode::OK, Json(views)).into_response()
        }
        ListRecipesResponse::Unavailable => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `POST /recipes` — création.
async fn create(
    user: AuthUser,
    State(state): State<RecipeState>,
    Json(body): Json<RecipeBody>,
) -> impl IntoResponse {
    let response = CreateRecipeHandler::new(state.recipes.as_ref())
        .handle(CreateRecipeCommand {
            household_id: user.household_id(),
            fields: body.into(),
        })
        .await;
    match response {
        CreateRecipeResponse::Created(recipe) => {
            (StatusCode::CREATED, Json(RecipeView::from(recipe))).into_response()
        }
        CreateRecipeResponse::Invalid(message) => invalid(message),
        CreateRecipeResponse::Unavailable => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `GET /recipes/:id` — détail.
async fn detail(
    user: AuthUser,
    State(state): State<RecipeState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let response = GetRecipeHandler::new(state.recipes.as_ref())
        .handle(GetRecipeQuery {
            household_id: user.household_id(),
            recipe_id: RecipeId::from(id),
        })
        .await;
    match response {
        GetRecipeResponse::Found(recipe) => {
            (StatusCode::OK, Json(RecipeView::from(recipe))).into_response()
        }
        GetRecipeResponse::NotFound => StatusCode::NOT_FOUND.into_response(),
        GetRecipeResponse::Unavailable => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `PUT /recipes/:id` — mise à jour (remplacement complet).
async fn update(
    user: AuthUser,
    State(state): State<RecipeState>,
    Path(id): Path<Uuid>,
    Json(body): Json<RecipeBody>,
) -> impl IntoResponse {
    let response = UpdateRecipeHandler::new(state.recipes.as_ref())
        .handle(UpdateRecipeCommand {
            household_id: user.household_id(),
            recipe_id: RecipeId::from(id),
            fields: body.into(),
        })
        .await;
    match response {
        UpdateRecipeResponse::Updated(recipe) => {
            (StatusCode::OK, Json(RecipeView::from(recipe))).into_response()
        }
        UpdateRecipeResponse::NotFound => StatusCode::NOT_FOUND.into_response(),
        UpdateRecipeResponse::Invalid(message) => invalid(message),
        UpdateRecipeResponse::Unavailable => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `DELETE /recipes/:id` — suppression.
async fn delete(
    user: AuthUser,
    State(state): State<RecipeState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let response = DeleteRecipeHandler::new(state.recipes.as_ref())
        .handle(DeleteRecipeCommand {
            household_id: user.household_id(),
            recipe_id: RecipeId::from(id),
        })
        .await;
    match response {
        DeleteRecipeResponse::Deleted => StatusCode::NO_CONTENT.into_response(),
        DeleteRecipeResponse::NotFound => StatusCode::NOT_FOUND.into_response(),
        DeleteRecipeResponse::Unavailable => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
