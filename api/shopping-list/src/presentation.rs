//! Couche présentation de `shopping-list` : routes Axum et DTO. Protégées par
//! l'extractor [`AuthUser`] (auth) et **scopées au foyer**.
//!
//! | Méthode | Route                          | Use case                          |
//! |---------|--------------------------------|-----------------------------------|
//! | GET     | `/shopping-list`               | lire la liste                     |
//! | POST    | `/shopping-list/generate`      | (re)générer depuis le calendrier  |
//! | POST    | `/shopping-list/items`         | ajouter une ligne à la main       |
//! | PATCH   | `/shopping-list/items/{id}`    | cocher / éditer une ligne         |
//! | DELETE  | `/shopping-list/items/{id}`    | supprimer une ligne               |
//! | DELETE  | `/shopping-list/checked`       | vider les lignes cochées          |

use std::sync::Arc;

use auth::presentation::AuthUser;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use chrono::NaiveDate;
use kernel::{ShoppingItemId, Unit};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::application::commands::{
    AddItemCommand, AddItemHandler, AddItemResponse, ClearCheckedCommand, ClearCheckedHandler,
    ClearCheckedResponse, DeleteItemCommand, DeleteItemHandler, DeleteItemResponse,
    GenerateListCommand, GenerateListHandler, GenerateListResponse, ReorderCommand, ReorderHandler,
    ReorderResponse, UpdateItemCommand, UpdateItemHandler, UpdateItemResponse,
};
use crate::application::queries::{GetListHandler, GetListQuery, GetListResponse};
use crate::domain::{
    PlannedIngredientsSource, ReferenceRepository, ShoppingItem, ShoppingListRepository,
};

/// État injecté dans les routes de la liste de courses.
#[derive(Clone)]
pub struct ShoppingListState {
    /// Repository de la liste.
    pub items: Arc<dyn ShoppingListRepository>,
    /// Référentiel d'ingrédients (poids moyens).
    pub references: Arc<dyn ReferenceRepository>,
    /// Projection des ingrédients planifiés.
    pub planned: Arc<dyn PlannedIngredientsSource>,
}

/// Sous-router de la liste de courses, monté par le `server`.
pub fn router(state: ShoppingListState) -> Router {
    Router::new()
        .route("/shopping-list", get(list))
        .route("/shopping-list/generate", post(generate))
        .route("/shopping-list/reorder", post(reorder))
        .route("/shopping-list/items", post(add))
        .route(
            "/shopping-list/items/{id}",
            axum::routing::patch(update).delete(remove),
        )
        .route("/shopping-list/checked", delete(clear_checked))
        .with_state(state)
}

// --- DTO ------------------------------------------------------------------

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

/// Une ligne exposée en réponse.
#[derive(Debug, Serialize)]
struct ItemView {
    id: Uuid,
    name: String,
    amount: f64,
    unit: Unit,
    category: Option<String>,
    checked: bool,
    /// `true` si la ligne vient de la génération (remplacée à la prochaine).
    generated: bool,
}

impl From<ShoppingItem> for ItemView {
    fn from(item: ShoppingItem) -> Self {
        Self {
            id: item.id.as_uuid(),
            name: item.name,
            amount: item.quantity.amount(),
            unit: item.quantity.unit(),
            category: item.category,
            checked: item.checked,
            generated: item.generated,
        }
    }
}

fn views(items: Vec<ShoppingItem>) -> Vec<ItemView> {
    items.into_iter().map(ItemView::from).collect()
}

/// Corps de la génération : plage de jours inclusive.
#[derive(Debug, Deserialize)]
struct GenerateBody {
    from: NaiveDate,
    to: NaiveDate,
}

/// Corps d'un ajout manuel.
#[derive(Debug, Deserialize)]
struct AddBody {
    name: String,
    amount: f64,
    unit: Unit,
}

/// Corps d'un réordonnancement : identifiants dans l'ordre voulu.
#[derive(Debug, Deserialize)]
struct ReorderBody {
    ids: Vec<Uuid>,
}

/// Corps d'une édition : tout est optionnel (champs absents = inchangés).
#[derive(Debug, Deserialize)]
struct UpdateBody {
    #[serde(default)]
    checked: Option<bool>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    amount: Option<f64>,
    #[serde(default)]
    unit: Option<Unit>,
}

// --- Handlers -------------------------------------------------------------

/// `GET /shopping-list` — lit la liste du foyer.
async fn list(user: AuthUser, State(state): State<ShoppingListState>) -> impl IntoResponse {
    let response = GetListHandler::new(state.items.as_ref())
        .handle(GetListQuery {
            household_id: user.household_id(),
        })
        .await;
    match response {
        GetListResponse::Loaded(items) => (StatusCode::OK, Json(views(items))).into_response(),
        GetListResponse::Unavailable => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `POST /shopping-list/generate` — (re)génère les lignes depuis le calendrier.
///
/// Idempotent : les lignes générées sont remplacées en bloc, les ajouts
/// manuels conservés, et l'état coché est repris.
async fn generate(
    user: AuthUser,
    State(state): State<ShoppingListState>,
    Json(body): Json<GenerateBody>,
) -> impl IntoResponse {
    let response = GenerateListHandler::new(
        state.items.as_ref(),
        state.references.as_ref(),
        state.planned.as_ref(),
    )
    .handle(GenerateListCommand {
        household_id: user.household_id(),
        from: body.from,
        to: body.to,
    })
    .await;

    match response {
        GenerateListResponse::Generated(items) => {
            (StatusCode::OK, Json(views(items))).into_response()
        }
        GenerateListResponse::InvalidRange => invalid("plage invalide (from après to)".to_owned()),
        GenerateListResponse::Unavailable => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `POST /shopping-list/reorder` — fixe l'ordre d'affichage des lignes.
async fn reorder(
    user: AuthUser,
    State(state): State<ShoppingListState>,
    Json(body): Json<ReorderBody>,
) -> impl IntoResponse {
    let ordered_ids = body.ids.into_iter().map(ShoppingItemId::from).collect();
    let response = ReorderHandler::new(state.items.as_ref())
        .handle(ReorderCommand {
            household_id: user.household_id(),
            ordered_ids,
        })
        .await;
    match response {
        ReorderResponse::Reordered => StatusCode::NO_CONTENT.into_response(),
        ReorderResponse::Unavailable => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `POST /shopping-list/items` — ajoute une ligne à la main.
async fn add(
    user: AuthUser,
    State(state): State<ShoppingListState>,
    Json(body): Json<AddBody>,
) -> impl IntoResponse {
    let response = AddItemHandler::new(state.items.as_ref())
        .handle(AddItemCommand {
            household_id: user.household_id(),
            name: body.name,
            amount: body.amount,
            unit: body.unit,
        })
        .await;
    match response {
        AddItemResponse::Added(item) => {
            (StatusCode::CREATED, Json(ItemView::from(item))).into_response()
        }
        AddItemResponse::Invalid(message) => invalid(message),
        AddItemResponse::Unavailable => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `PATCH /shopping-list/items/:id` — coche ou édite une ligne.
async fn update(
    user: AuthUser,
    State(state): State<ShoppingListState>,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateBody>,
) -> impl IntoResponse {
    let response = UpdateItemHandler::new(state.items.as_ref())
        .handle(UpdateItemCommand {
            household_id: Some(user.household_id()),
            id: Some(ShoppingItemId::from(id)),
            checked: body.checked,
            name: body.name,
            amount: body.amount,
            unit: body.unit,
        })
        .await;
    match response {
        UpdateItemResponse::Updated(item) => {
            (StatusCode::OK, Json(ItemView::from(item))).into_response()
        }
        UpdateItemResponse::NotFound => StatusCode::NOT_FOUND.into_response(),
        UpdateItemResponse::Invalid(message) => invalid(message),
        UpdateItemResponse::Unavailable => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `DELETE /shopping-list/items/:id` — supprime une ligne.
async fn remove(
    user: AuthUser,
    State(state): State<ShoppingListState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let response = DeleteItemHandler::new(state.items.as_ref())
        .handle(DeleteItemCommand {
            household_id: user.household_id(),
            id: ShoppingItemId::from(id),
        })
        .await;
    match response {
        DeleteItemResponse::Deleted => StatusCode::NO_CONTENT.into_response(),
        DeleteItemResponse::NotFound => StatusCode::NOT_FOUND.into_response(),
        DeleteItemResponse::Unavailable => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `DELETE /shopping-list/checked` — vide les lignes cochées.
async fn clear_checked(
    user: AuthUser,
    State(state): State<ShoppingListState>,
) -> impl IntoResponse {
    let response = ClearCheckedHandler::new(state.items.as_ref())
        .handle(ClearCheckedCommand {
            household_id: user.household_id(),
        })
        .await;
    match response {
        ClearCheckedResponse::Cleared(_) => StatusCode::NO_CONTENT.into_response(),
        ClearCheckedResponse::Unavailable => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}
