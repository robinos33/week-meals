//! La liste de courses d'un foyer : entité, origine des lignes et ports de
//! persistance. Aucune dépendance à SQLx/Axum (règle de convention).
//!
//! Un foyer a **une** liste courante. Chaque ligne est soit **générée** depuis
//! le calendrier (`generated`), soit **ajoutée à la main**. Cette distinction
//! est ce qui rend la régénération idempotente : on remplace en bloc les
//! lignes générées, les ajouts manuels survivent (cf.
//! [`ShoppingListRepository::replace_generated`]).

use chrono::NaiveDate;
use kernel::{HouseholdId, Quantity, RepositoryError, ShoppingItemId};

use super::{PlannedIngredient, ReferenceCatalog};

/// Une ligne de la liste de courses.
#[derive(Debug, Clone, PartialEq)]
pub struct ShoppingItem {
    /// Identifiant de la ligne.
    pub id: ShoppingItemId,
    /// Foyer propriétaire (scope).
    pub household_id: HouseholdId,
    /// Nom affiché.
    pub name: String,
    /// Quantité à acheter.
    pub quantity: Quantity,
    /// Rayon, connu seulement pour les ingrédients référencés.
    pub category: Option<String>,
    /// Coché en magasin.
    pub checked: bool,
    /// `true` si la ligne vient de la génération (donc remplaçable).
    pub generated: bool,
    /// Ordre d'affichage.
    pub position: i32,
}

impl ShoppingItem {
    /// Construit une ligne ajoutée à la main (jamais remplacée par une
    /// régénération).
    #[must_use]
    pub fn manual(household_id: HouseholdId, name: impl Into<String>, quantity: Quantity) -> Self {
        Self {
            id: ShoppingItemId::new(),
            household_id,
            name: name.into().trim().to_owned(),
            quantity,
            category: None,
            checked: false,
            generated: false,
            position: 0,
        }
    }
}

/// Port de persistance de la liste. Tout est scopé à un [`HouseholdId`].
#[async_trait::async_trait]
pub trait ShoppingListRepository: Send + Sync {
    /// Liste complète du foyer, ordonnée (générées puis manuelles, par position).
    async fn list(&self, household_id: HouseholdId) -> Result<Vec<ShoppingItem>, RepositoryError>;

    /// Récupère une ligne du foyer.
    async fn find(
        &self,
        household_id: HouseholdId,
        id: ShoppingItemId,
    ) -> Result<Option<ShoppingItem>, RepositoryError>;

    /// Remplace **en bloc** les lignes générées du foyer par `items`, en une
    /// transaction. Les lignes manuelles ne sont pas touchées : régénérer deux
    /// fois la même plage donne donc la même liste (idempotence).
    async fn replace_generated(
        &self,
        household_id: HouseholdId,
        items: &[ShoppingItem],
    ) -> Result<(), RepositoryError>;

    /// Ajoute une ligne.
    async fn add(&self, item: &ShoppingItem) -> Result<(), RepositoryError>;

    /// Met à jour une ligne existante.
    ///
    /// # Errors
    /// [`RepositoryError::NotFound`] si la ligne n'existe pas dans le foyer.
    async fn update(&self, item: &ShoppingItem) -> Result<(), RepositoryError>;

    /// Supprime une ligne.
    ///
    /// # Errors
    /// [`RepositoryError::NotFound`] si la ligne n'existe pas dans le foyer.
    async fn delete(
        &self,
        household_id: HouseholdId,
        id: ShoppingItemId,
    ) -> Result<(), RepositoryError>;

    /// Supprime toutes les lignes cochées du foyer, et renvoie leur nombre.
    async fn clear_checked(&self, household_id: HouseholdId) -> Result<u64, RepositoryError>;
}

/// Port de lecture du référentiel d'ingrédients (catalogue global, seedé
/// depuis `data/ingredients.yaml`).
#[async_trait::async_trait]
pub trait ReferenceRepository: Send + Sync {
    /// Charge tout le catalogue (il tient largement en mémoire).
    async fn catalog(&self) -> Result<ReferenceCatalog, RepositoryError>;
}

/// Port de lecture des ingrédients planifiés sur une plage de jours.
///
/// Projection **en lecture seule** au travers du calendrier et des recettes :
/// le domaine `shopping-list` n'a pas à connaître ces domaines, il consomme
/// juste la liste plate des ingrédients à acheter.
#[async_trait::async_trait]
pub trait PlannedIngredientsSource: Send + Sync {
    /// Ingrédients de toutes les recettes planifiées du foyer entre `from` et
    /// `to` (bornes incluses). Une recette planifiée deux fois compte double.
    async fn planned(
        &self,
        household_id: HouseholdId,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<PlannedIngredient>, RepositoryError>;
}
