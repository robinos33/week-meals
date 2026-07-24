//! Couche présentation de `recipes` : routes Axum et DTO. Les routes de gestion
//! des recettes sont protégées par l'extractor [`AuthUser`] (auth) et **scopées
//! au foyer** de l'utilisateur connecté. Exception : le dépôt et le service des
//! fichiers photo du volume (ADR-0009) ne passent pas par la session — le dépôt
//! est autorisé par le jeton du presign, le service sert une clé UUID opaque.
//!
//! | Méthode | Route                     | Use case                      |
//! |---------|---------------------------|-------------------------------|
//! | GET     | `/recipes`                | liste / recherche (`?search`) |
//! | POST    | `/recipes`                | création                      |
//! | POST    | `/recipes/scrape`         | import par URL → brouillon (#61) |
//! | POST    | `/recipes/photos/presign` | présignature upload photo     |
//! | PUT     | `/recipes/photos/:file`   | dépôt fichier (volume, ADR-0009) |
//! | GET     | `/recipes/photos/:file`   | service fichier (volume)      |
//! | GET     | `/recipes/:id`            | détail                        |
//! | PUT     | `/recipes/:id`            | mise à jour                   |
//! | DELETE  | `/recipes/:id`            | suppression                   |

use std::sync::Arc;

use auth::presentation::AuthUser;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
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
use crate::application::scrape::{ScrapeRecipeCommand, ScrapeRecipeHandler, ScrapeRecipeResponse};
use crate::application::IngredientInput;
use crate::domain::{
    PhotoError, PhotoStorage, Recipe, RecipeRepository, RecipeScraper, ScrapeError, ScrapedRecipe,
};
use crate::infrastructure::VolumePhotoStorage;

/// État injecté dans les routes recettes.
#[derive(Clone)]
pub struct RecipeState {
    /// Repository des recettes.
    pub recipes: Arc<dyn RecipeRepository>,
    /// Stockage des photos (présignature). `None` si aucun backend n'est
    /// configuré : la route de présignature répond alors `503`.
    pub photos: Option<Arc<dyn PhotoStorage>>,
    /// Stockage volume concret, présent seulement quand c'est *lui* le backend
    /// actif (cf. ADR-0009) : il sert les routes `PUT`/`GET` des fichiers, que
    /// R2 (quand configuré) n'utilise pas. `None` en mode R2 ou sans stockage.
    pub local_photos: Option<Arc<VolumePhotoStorage>>,
    /// Scraper d'import par URL (#61). Garde SSRF côté implémentation.
    pub scraper: Arc<dyn RecipeScraper>,
}

/// Sous-router des recettes, monté par le `server`.
pub fn router(state: RecipeState) -> Router {
    Router::new()
        .route("/recipes", get(list).post(create))
        // Routes statiques : segment fixe prioritaire sur `/recipes/{id}`.
        .route("/recipes/photos/presign", post(presign_photo))
        // Backend volume (ADR-0009) : dépôt et service des fichiers. `{filename}`
        // reste plus spécifique que `/recipes/{id}` (préfixe fixe `photos/`).
        // Le corps du `PUT` est plafonné pour ne pas laisser remplir le volume.
        .route(
            "/recipes/photos/{filename}",
            get(serve_photo)
                .put(upload_photo)
                .layer(DefaultBodyLimit::max(8 * 1024 * 1024)),
        )
        .route("/recipes/scrape", post(scrape_recipe))
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
    /// Nombre de fois cuisinée (#58) : « Cuisiné X fois » sur la fiche, et tri
    /// du podium 🥇🥈🥉 dans la grille.
    cooked_count: u32,
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
            cooked_count: recipe.cooked_count,
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

// --- Photos ---------------------------------------------------------------

/// Corps de la demande de présignature.
#[derive(Debug, Deserialize)]
struct PresignBody {
    /// Type MIME de l'image à déposer (`image/jpeg`, `image/png`, `image/webp`).
    content_type: String,
}

/// Réponse de présignature : où déposer le fichier, et l'URL à stocker ensuite.
#[derive(Debug, Serialize)]
struct PresignView {
    upload_url: String,
    public_url: String,
}

/// `POST /recipes/photos/presign` — présigne un upload direct au stockage.
///
/// Le client dépose ensuite le fichier sur `upload_url` (PUT direct à R2), puis
/// enregistre `public_url` dans la recette. `503` si le stockage n'est pas
/// configuré (dev sans R2), `422` si le type d'image n'est pas pris en charge.
async fn presign_photo(
    _user: AuthUser,
    State(state): State<RecipeState>,
    Json(body): Json<PresignBody>,
) -> impl IntoResponse {
    let Some(photos) = state.photos.as_ref() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorBody {
                error: "stockage photo non configuré".to_owned(),
            }),
        )
            .into_response();
    };
    match photos.presign_upload(&body.content_type).await {
        Ok(upload) => (
            StatusCode::OK,
            Json(PresignView {
                upload_url: upload.upload_url,
                public_url: upload.public_url,
            }),
        )
            .into_response(),
        Err(PhotoError::UnsupportedType(_)) => {
            invalid("type d'image non pris en charge (jpeg, png ou webp)".to_owned())
        }
        Err(PhotoError::Unauthorized | PhotoError::Backend(_)) => {
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Query de dépôt : le jeton émis par la présignature (backend volume).
#[derive(Debug, Deserialize)]
struct UploadQuery {
    token: String,
}

/// `PUT /recipes/photos/{filename}` — dépose les octets d'un upload présigné sur
/// le volume (backend volume uniquement, cf. ADR-0009).
///
/// Pas de session : le `PUT` est autorisé par le `token` du presign (le front
/// dépose sans cookie, comme vers R2). `404` si le backend volume n'est pas
/// actif (mode R2 ou sans stockage), `403` si le jeton est invalide/expiré,
/// `422` si le nom de fichier n'est pas une image prise en charge.
async fn upload_photo(
    State(state): State<RecipeState>,
    Path(filename): Path<String>,
    Query(query): Query<UploadQuery>,
    body: Bytes,
) -> impl IntoResponse {
    let Some(storage) = state.local_photos.as_ref() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match storage.store(&filename, &query.token, &body).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(PhotoError::UnsupportedType(_)) => {
            invalid("type d'image non pris en charge (jpeg, png ou webp)".to_owned())
        }
        Err(PhotoError::Unauthorized) => StatusCode::FORBIDDEN.into_response(),
        Err(PhotoError::Backend(_)) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `GET /recipes/photos/{filename}` — sert un fichier photo du volume (backend
/// volume uniquement, cf. ADR-0009).
///
/// Public (pas de session) : la balise `<img>` charge l'URL directement, et la
/// clé est un UUID opaque. Contenu adressé par UUID, donc immuable — mis en
/// cache un an. `404` si le backend volume n'est pas actif ou le fichier absent.
async fn serve_photo(
    State(state): State<RecipeState>,
    Path(filename): Path<String>,
) -> impl IntoResponse {
    let Some(storage) = state.local_photos.as_ref() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    match storage.load(&filename).await {
        Some((content_type, bytes)) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, content_type),
                (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
            ],
            bytes,
        )
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

// --- Import par URL (#61) -------------------------------------------------

/// Corps de la demande d'import : l'URL de la page de recette.
#[derive(Debug, Deserialize)]
struct ScrapeBody {
    url: String,
}

/// Brouillon renvoyé au front : mêmes champs que le corps de création, pour être
/// injecté directement dans l'état du formulaire (à corriger avant d'enregistrer).
#[derive(Debug, Serialize)]
struct RecipeDraftView {
    title: String,
    prep_time_min: Option<u32>,
    cook_time_min: Option<u32>,
    photo: Option<String>,
    ingredients: Vec<IngredientView>,
    steps: Vec<String>,
}

impl From<ScrapedRecipe> for RecipeDraftView {
    fn from(recipe: ScrapedRecipe) -> Self {
        Self {
            title: recipe.title,
            prep_time_min: recipe.prep_time_min,
            cook_time_min: recipe.cook_time_min,
            photo: recipe.photo,
            ingredients: recipe
                .ingredients
                .into_iter()
                .map(|i| IngredientView {
                    name: i.name,
                    amount: i.amount,
                    unit: i.unit,
                })
                .collect(),
            steps: recipe.steps,
        }
    }
}

/// Statut HTTP d'un échec d'import : `502` quand la page distante est en cause,
/// `422` quand c'est l'URL fournie (invalide, interdite, sans recette).
fn scrape_status(error: &ScrapeError) -> StatusCode {
    match error {
        ScrapeError::Unreachable => StatusCode::BAD_GATEWAY,
        _ => StatusCode::UNPROCESSABLE_ENTITY,
    }
}

/// `POST /recipes/scrape` — extrait un brouillon de recette d'une URL (#61).
///
/// Le serveur va chercher l'URL : garde SSRF côté implémentation (https,
/// IP publiques, sans redirection, taille bornée). Ne crée jamais la recette —
/// il renvoie un brouillon que le front prérempli dans le formulaire.
async fn scrape_recipe(
    _user: AuthUser,
    State(state): State<RecipeState>,
    Json(body): Json<ScrapeBody>,
) -> impl IntoResponse {
    let response = ScrapeRecipeHandler::new(state.scraper.as_ref())
        .handle(ScrapeRecipeCommand { url: body.url })
        .await;
    match response {
        ScrapeRecipeResponse::Drafted(recipe) => {
            (StatusCode::OK, Json(RecipeDraftView::from(recipe))).into_response()
        }
        ScrapeRecipeResponse::Rejected(error) => (
            scrape_status(&error),
            Json(ErrorBody {
                error: error.to_string(),
            }),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::InMemoryRecipes;

    /// Scraper inerte pour construire un état de test.
    struct NoopScraper;

    #[async_trait::async_trait]
    impl RecipeScraper for NoopScraper {
        async fn scrape(&self, _url: &str) -> Result<ScrapedRecipe, ScrapeError> {
            Err(ScrapeError::NoRecipe)
        }
    }

    #[test]
    fn router_mounts_without_route_conflict() {
        // Construire le routeur suffit : Axum panique à l'insertion si
        // `/recipes/scrape` entrait en conflit avec `/recipes/{id}`.
        let state = RecipeState {
            recipes: Arc::new(InMemoryRecipes::default()),
            photos: None,
            local_photos: None,
            scraper: Arc::new(NoopScraper),
        };
        let _ = router(state);
    }
}
