//! Couche domaine de `recipes` : entités, value objects, traits de repository
//! et services purs. Aucune dépendance à SQLx/Axum (règle de convention).
//!
//! Modélise une recette (cf. [issue #9], [plan.md — Modèle métier]) :
//! titre, temps de préparation/cuisson, photo optionnelle, ingrédients
//! (`name` + [`Quantity`]) et étapes de préparation ordonnées.
//!
//! [issue #9]: https://github.com/robinos33/week-meals/issues/9
//! [plan.md — Modèle métier]: ../../../docs/plan.md

use kernel::{HouseholdId, Quantity, RecipeId, RepositoryError, Unit};

/// Un ingrédient d'une recette : un nom libre et une [`Quantity`].
///
/// Le nom n'est pas contraint par le référentiel `IngredientReference` : un
/// ingrédient sans poids moyen connu reste valide, il ne sera simplement pas
/// reconverti en pièces lors de la génération de liste de courses.
#[derive(Debug, Clone, PartialEq)]
pub struct RecipeIngredient {
    /// Nom de l'ingrédient (ex. « courgette »).
    pub name: String,
    /// Quantité (montant + unité).
    pub quantity: Quantity,
}

impl RecipeIngredient {
    /// Construit un ingrédient. Le nom est nettoyé (trim).
    ///
    /// # Errors
    /// Renvoie [`RecipeError::EmptyIngredientName`] si le nom est vide.
    pub fn new(name: impl Into<String>, quantity: Quantity) -> Result<Self, RecipeError> {
        let name = name.into().trim().to_owned();
        if name.is_empty() {
            return Err(RecipeError::EmptyIngredientName);
        }
        Ok(Self { name, quantity })
    }
}

/// Une recette : agrégat racine du domaine `recipes`, scopé à un foyer.
#[derive(Debug, Clone, PartialEq)]
pub struct Recipe {
    /// Identifiant de la recette.
    pub id: RecipeId,
    /// Foyer propriétaire (toutes les données sont scopées au foyer).
    pub household_id: HouseholdId,
    /// Titre.
    pub title: String,
    /// Photo : URL R2 ou chemin de seed. Optionnelle.
    pub photo: Option<String>,
    /// Temps de préparation en minutes. Optionnel.
    pub prep_time_min: Option<u32>,
    /// Temps de cuisson en minutes. Optionnel.
    pub cook_time_min: Option<u32>,
    /// Ingrédients.
    pub ingredients: Vec<RecipeIngredient>,
    /// Étapes de préparation, dans l'ordre.
    pub steps: Vec<String>,
    /// Nombre de fois où la recette a été cuisinée (#58). Incrémenté à la
    /// génération de la liste de courses, jamais saisi par l'utilisateur : les
    /// constructeurs le posent à `0`, la persistance l'injecte via
    /// [`Recipe::with_cooked_count`].
    pub cooked_count: u32,
}

/// Borne haute d'un temps de préparation / cuisson, en minutes.
///
/// Les temps sont persistés dans un `integer` Postgres (cf. ADR-0003 : la DB
/// est la source de vérité) : au-delà, l'écriture violerait la contrainte
/// `check (>= 0)` après troncature. On valide donc ici, pour que l'entrée
/// hors bornes ressorte en `Invalid` (422) plutôt qu'en panne technique (500).
pub const MAX_TIME_MIN: u32 = i32::MAX as u32;

impl Recipe {
    /// Construit une recette en validant ses invariants (titre non vide, temps
    /// dans les bornes).
    ///
    /// Génère un nouvel identifiant. Le titre est nettoyé (trim).
    ///
    /// # Errors
    /// Voir [`Recipe::from_parts`].
    pub fn new(
        household_id: HouseholdId,
        title: impl Into<String>,
        prep_time_min: Option<u32>,
        cook_time_min: Option<u32>,
        photo: Option<String>,
        ingredients: Vec<RecipeIngredient>,
        steps: Vec<String>,
    ) -> Result<Self, RecipeError> {
        Self::from_parts(
            RecipeId::new(),
            household_id,
            title,
            prep_time_min,
            cook_time_min,
            photo,
            ingredients,
            steps,
        )
    }

    /// Reconstitue une recette dont l'identifiant est connu — pour une mise à
    /// jour (l'`id` est fourni par l'appelant) ou une lecture depuis la
    /// persistance. Valide et nettoie le titre comme [`Recipe::new`].
    ///
    /// # Errors
    /// - [`RecipeError::EmptyTitle`] si le titre est vide ;
    /// - [`RecipeError::TimeOutOfRange`] si un temps dépasse [`MAX_TIME_MIN`].
    #[allow(clippy::too_many_arguments)]
    pub fn from_parts(
        id: RecipeId,
        household_id: HouseholdId,
        title: impl Into<String>,
        prep_time_min: Option<u32>,
        cook_time_min: Option<u32>,
        photo: Option<String>,
        ingredients: Vec<RecipeIngredient>,
        steps: Vec<String>,
    ) -> Result<Self, RecipeError> {
        let title = title.into().trim().to_owned();
        if title.is_empty() {
            return Err(RecipeError::EmptyTitle);
        }
        if [prep_time_min, cook_time_min]
            .into_iter()
            .flatten()
            .any(|minutes| minutes > MAX_TIME_MIN)
        {
            return Err(RecipeError::TimeOutOfRange);
        }
        Ok(Self {
            id,
            household_id,
            title,
            photo,
            prep_time_min,
            cook_time_min,
            ingredients,
            steps,
            cooked_count: 0,
        })
    }

    /// Fixe le compteur « cuisiné X fois » (#58). Réservé à la persistance, qui
    /// reconstitue la recette avec sa valeur stockée ; les constructeurs le
    /// laissent à `0`.
    #[must_use]
    pub fn with_cooked_count(mut self, cooked_count: u32) -> Self {
        self.cooked_count = cooked_count;
        self
    }
}

/// Violation d'un invariant du domaine `recipes`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RecipeError {
    /// Le titre d'une recette ne peut pas être vide.
    #[error("recipe title must not be empty")]
    EmptyTitle,
    /// Le nom d'un ingrédient ne peut pas être vide.
    #[error("ingredient name must not be empty")]
    EmptyIngredientName,
    /// Un temps de préparation / cuisson dépasse la borne persistable.
    #[error("recipe time must not exceed {MAX_TIME_MIN} minutes")]
    TimeOutOfRange,
}

/// Extension de fichier autorisée pour un type MIME d'image. Contrat partagé
/// entre la présentation (validation) et l'infrastructure (nom de l'objet).
/// `None` = type non pris en charge.
#[must_use]
pub fn photo_extension(content_type: &str) -> Option<&'static str> {
    match content_type {
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/webp" => Some("webp"),
        _ => None,
    }
}

/// Coordonnées d'un upload photo présigné.
///
/// Le client dépose le fichier directement sur `upload_url` (PUT, sans passer
/// par l'API), puis stocke `public_url` dans la recette (champ `photo`).
#[derive(Debug, Clone, PartialEq)]
pub struct PhotoUpload {
    /// URL présignée où déposer le fichier (PUT direct au stockage).
    pub upload_url: String,
    /// URL publique finale, à persister dans la recette.
    pub public_url: String,
}

/// Échec de présignature d'un upload photo.
#[derive(Debug, thiserror::Error)]
pub enum PhotoError {
    /// Type MIME non pris en charge (cf. [`photo_extension`]).
    #[error("unsupported photo content type: {0}")]
    UnsupportedType(String),
    /// Panne technique du stockage (S3/R2).
    #[error("photo storage error: {0}")]
    Backend(String),
}

/// Port de stockage des photos (S3-compatible : Cloudflare R2 en prod, MinIO en
/// dev). Le domaine ne connaît que ce trait ; l'implémentation vit dans
/// l'infrastructure. Seule la **présignature** est exposée : l'octet du fichier
/// ne transite jamais par l'API.
#[async_trait::async_trait]
pub trait PhotoStorage: Send + Sync {
    /// Présigne un upload pour un `content_type` d'image donné.
    ///
    /// # Errors
    /// - [`PhotoError::UnsupportedType`] si le type MIME n'est pas une image
    ///   prise en charge ;
    /// - [`PhotoError::Backend`] si la présignature échoue.
    async fn presign_upload(&self, content_type: &str) -> Result<PhotoUpload, PhotoError>;
}

/// Port de persistance des recettes. Implémenté dans la couche infrastructure
/// (SQLx) ; le domaine ne connaît que ce trait.
///
/// Toutes les lectures/écritures sont scopées à un [`HouseholdId`] : une
/// recette n'est jamais accessible hors de son foyer.
#[async_trait::async_trait]
pub trait RecipeRepository: Send + Sync {
    /// Persiste une nouvelle recette.
    async fn create(&self, recipe: &Recipe) -> Result<(), RepositoryError>;

    /// Récupère une recette du foyer par son identifiant, si elle existe.
    async fn find(
        &self,
        household_id: HouseholdId,
        id: RecipeId,
    ) -> Result<Option<Recipe>, RepositoryError>;

    /// Liste les recettes du foyer.
    async fn list(&self, household_id: HouseholdId) -> Result<Vec<Recipe>, RepositoryError>;

    /// Recherche les recettes du foyer dont le titre contient `query`
    /// (insensible à la casse). Une requête vide équivaut à [`Self::list`].
    async fn search(
        &self,
        household_id: HouseholdId,
        query: &str,
    ) -> Result<Vec<Recipe>, RepositoryError>;

    /// Met à jour une recette existante.
    ///
    /// # Errors
    /// [`RepositoryError::NotFound`] si la recette n'existe pas dans le foyer.
    async fn update(&self, recipe: &Recipe) -> Result<(), RepositoryError>;

    /// Supprime une recette du foyer.
    ///
    /// # Errors
    /// [`RepositoryError::NotFound`] si la recette n'existe pas dans le foyer.
    async fn delete(&self, household_id: HouseholdId, id: RecipeId) -> Result<(), RepositoryError>;
}

// --- Import d'une recette par URL (scraping, #61) -------------------------

/// Un ingrédient extrait d'une page web. Le découpage quantité/unité est
/// heuristique (« 2 c. à soupe d'huile ») : c'est un brouillon à relire.
#[derive(Debug, Clone, PartialEq)]
pub struct ScrapedIngredient {
    /// Nom libre.
    pub name: String,
    /// Montant, dans l'unité `unit`.
    pub amount: f64,
    /// Unité.
    pub unit: Unit,
}

/// Une recette extraite d'une page web : un **brouillon à relire**, à la forme
/// du formulaire (mêmes champs que `RecipeFields`). Jamais persisté tel quel —
/// il prérempli un formulaire que l'utilisateur corrige avant d'enregistrer.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScrapedRecipe {
    /// Titre.
    pub title: String,
    /// Temps de préparation en minutes (optionnel).
    pub prep_time_min: Option<u32>,
    /// Temps de cuisson en minutes (optionnel).
    pub cook_time_min: Option<u32>,
    /// Photo (URL distante telle que publiée par le site).
    pub photo: Option<String>,
    /// Ingrédients.
    pub ingredients: Vec<ScrapedIngredient>,
    /// Étapes de préparation, ordonnées.
    pub steps: Vec<String>,
}

/// Échec d'un import par URL. Les messages sont destinés à l'utilisateur.
#[derive(Debug, thiserror::Error)]
pub enum ScrapeError {
    /// L'entrée n'est pas une URL exploitable.
    #[error("l'adresse fournie n'est pas une URL valide")]
    InvalidUrl,
    /// Schéma non https (garde SSRF côté serveur).
    #[error("seules les adresses https sont acceptées")]
    NotHttps,
    /// Cible interdite : loopback, IP privée, link-local… (garde SSRF).
    #[error("cette adresse n'est pas autorisée")]
    Blocked,
    /// Page injoignable (réseau, DNS, ou statut d'erreur).
    #[error("page injoignable")]
    Unreachable,
    /// Réponse trop volumineuse pour être analysée.
    #[error("la page est trop volumineuse pour être analysée")]
    TooLarge,
    /// Aucune recette JSON-LD (schema.org) sur la page.
    #[error("aucune recette n'a été trouvée sur cette page")]
    NoRecipe,
}

/// Port d'import d'une recette depuis une URL. L'implémentation vit dans
/// l'infrastructure ; le domaine ne connaît que ce trait.
///
/// Exposé en API, c'est le **serveur** qui récupère l'URL fournie par le
/// client : l'implémentation garde donc contre le SSRF (https uniquement, IP
/// publiques vérifiées, redirections désactivées, taille bornée).
#[async_trait::async_trait]
pub trait RecipeScraper: Send + Sync {
    /// Récupère `url` et en extrait un brouillon de recette.
    ///
    /// # Errors
    /// Voir [`ScrapeError`] : URL invalide ou non https, cible interdite, page
    /// injoignable, trop volumineuse, ou sans recette JSON-LD.
    async fn scrape(&self, url: &str) -> Result<ScrapedRecipe, ScrapeError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernel::Unit;

    fn ingredient(name: &str, amount: f64, unit: Unit) -> RecipeIngredient {
        RecipeIngredient::new(name, Quantity::new(amount, unit).unwrap()).unwrap()
    }

    #[test]
    fn builds_a_recipe_with_a_fresh_id() {
        let household = HouseholdId::new();
        let recipe = Recipe::new(
            household,
            "Ratatouille",
            Some(25),
            Some(45),
            None,
            vec![
                ingredient("courgette", 600.0, Unit::G),
                ingredient("gousse d'ail", 3.0, Unit::Piece),
            ],
            vec![
                "Émincer l'oignon.".to_owned(),
                "Laisser mijoter.".to_owned(),
            ],
        )
        .unwrap();

        assert_eq!(recipe.household_id, household);
        assert_eq!(recipe.title, "Ratatouille");
        assert_eq!(recipe.ingredients.len(), 2);
        assert_eq!(recipe.steps.len(), 2);
    }

    #[test]
    fn rejects_a_time_beyond_the_persistable_range() {
        let error = Recipe::new(
            HouseholdId::new(),
            "Bœuf de sept heures",
            Some(MAX_TIME_MIN + 1),
            None,
            None,
            vec![],
            vec![],
        )
        .unwrap_err();
        assert_eq!(error, RecipeError::TimeOutOfRange);
    }

    #[test]
    fn accepts_a_time_at_the_boundary() {
        let recipe = Recipe::new(
            HouseholdId::new(),
            "Bœuf de sept heures",
            Some(MAX_TIME_MIN),
            None,
            None,
            vec![],
            vec![],
        )
        .unwrap();
        assert_eq!(recipe.prep_time_min, Some(MAX_TIME_MIN));
    }

    #[test]
    fn trims_the_title() {
        let recipe = Recipe::new(
            HouseholdId::new(),
            "  Tarte aux pommes  ",
            None,
            None,
            None,
            vec![],
            vec![],
        )
        .unwrap();
        assert_eq!(recipe.title, "Tarte aux pommes");
    }

    #[test]
    fn rejects_a_blank_title() {
        let err =
            Recipe::new(HouseholdId::new(), "   ", None, None, None, vec![], vec![]).unwrap_err();
        assert_eq!(err, RecipeError::EmptyTitle);
    }

    #[test]
    fn rejects_a_blank_ingredient_name() {
        let err = RecipeIngredient::new("  ", Quantity::new(1.0, Unit::G).unwrap()).unwrap_err();
        assert_eq!(err, RecipeError::EmptyIngredientName);
    }
}
