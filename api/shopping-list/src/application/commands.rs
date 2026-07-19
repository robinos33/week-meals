//! Use cases d'écriture de la liste de courses : génération depuis le
//! calendrier, ajout manuel, édition, suppression, vidage des cochés.

use std::collections::HashSet;

use chrono::NaiveDate;
use kernel::{HouseholdId, Quantity, QuantityError, RepositoryError, ShoppingItemId, Unit};

use crate::domain::reference::normalize_name;
use crate::domain::{
    aggregate_purchases, PlannedIngredientsSource, ReferenceRepository, ShoppingItem,
    ShoppingListRepository,
};

// --- Génération -----------------------------------------------------------

/// Command : (re)génère les lignes de la liste depuis le calendrier, sur une
/// plage de jours inclusive.
#[derive(Debug, Clone)]
pub struct GenerateListCommand {
    /// Foyer propriétaire (scope).
    pub household_id: HouseholdId,
    /// Premier jour (inclus).
    pub from: NaiveDate,
    /// Dernier jour (inclus).
    pub to: NaiveDate,
}

/// Résultat d'une génération.
#[derive(Debug)]
pub enum GenerateListResponse {
    /// Liste complète après génération (lignes générées + ajouts manuels).
    Generated(Vec<ShoppingItem>),
    /// La plage est invalide (`from` après `to`).
    InvalidRange,
    /// Panne technique.
    Unavailable,
}

/// Handler de la génération.
///
/// Enchaîne : lecture du référentiel → lecture des ingrédients planifiés →
/// agrégation/conversion (service pur du domaine) → remplacement en bloc des
/// lignes générées.
pub struct GenerateListHandler<'a> {
    items: &'a dyn ShoppingListRepository,
    references: &'a dyn ReferenceRepository,
    planned: &'a dyn PlannedIngredientsSource,
}

impl<'a> GenerateListHandler<'a> {
    /// Construit le handler.
    #[must_use]
    pub fn new(
        items: &'a dyn ShoppingListRepository,
        references: &'a dyn ReferenceRepository,
        planned: &'a dyn PlannedIngredientsSource,
    ) -> Self {
        Self {
            items,
            references,
            planned,
        }
    }

    /// Exécute la génération. Ne renvoie jamais d'erreur.
    pub async fn handle(&self, command: GenerateListCommand) -> GenerateListResponse {
        if command.from > command.to {
            return GenerateListResponse::InvalidRange;
        }

        let Ok(catalog) = self.references.catalog().await else {
            return GenerateListResponse::Unavailable;
        };
        let Ok(planned) = self
            .planned
            .planned(command.household_id, command.from, command.to)
            .await
        else {
            return GenerateListResponse::Unavailable;
        };
        let Ok(existing) = self.items.list(command.household_id).await else {
            return GenerateListResponse::Unavailable;
        };

        // Ce qui était déjà coché le reste après régénération : sinon, ajouter
        // une recette à la semaine ferait « décocher » tout le caddie.
        let checked: HashSet<String> = existing
            .iter()
            .filter(|item| item.generated && item.checked)
            .map(|item| normalize_name(&item.name))
            .collect();

        let purchases = aggregate_purchases(&planned, &catalog);
        let items: Vec<ShoppingItem> = purchases
            .into_iter()
            .enumerate()
            .map(|(index, purchase)| ShoppingItem {
                id: ShoppingItemId::new(),
                household_id: command.household_id,
                checked: checked.contains(&normalize_name(&purchase.name)),
                name: purchase.name,
                quantity: purchase.quantity,
                category: purchase.category,
                generated: true,
                position: i32::try_from(index).unwrap_or(i32::MAX),
            })
            .collect();

        if self
            .items
            .replace_generated(command.household_id, &items)
            .await
            .is_err()
        {
            return GenerateListResponse::Unavailable;
        }

        match self.items.list(command.household_id).await {
            Ok(all) => GenerateListResponse::Generated(all),
            Err(_) => GenerateListResponse::Unavailable,
        }
    }
}

// --- Ajout manuel ---------------------------------------------------------

/// Command : ajoute une ligne à la main.
#[derive(Debug, Clone)]
pub struct AddItemCommand {
    /// Foyer propriétaire (scope).
    pub household_id: HouseholdId,
    /// Nom libre.
    pub name: String,
    /// Montant.
    pub amount: f64,
    /// Unité.
    pub unit: Unit,
}

/// Résultat d'un ajout.
#[derive(Debug)]
pub enum AddItemResponse {
    /// Ligne ajoutée.
    Added(ShoppingItem),
    /// Entrée invalide (nom vide, quantité ≤ 0).
    Invalid(String),
    /// Panne technique.
    Unavailable,
}

/// Handler de l'ajout manuel.
pub struct AddItemHandler<'a> {
    items: &'a dyn ShoppingListRepository,
}

impl<'a> AddItemHandler<'a> {
    /// Construit le handler.
    #[must_use]
    pub fn new(items: &'a dyn ShoppingListRepository) -> Self {
        Self { items }
    }

    /// Exécute l'ajout. Ne renvoie jamais d'erreur.
    pub async fn handle(&self, command: AddItemCommand) -> AddItemResponse {
        if command.name.trim().is_empty() {
            return AddItemResponse::Invalid("le nom ne peut pas être vide".to_owned());
        }
        let quantity = match Quantity::new(command.amount, command.unit) {
            Ok(quantity) => quantity,
            Err(error) => return AddItemResponse::Invalid(quantity_error(error)),
        };

        let item = ShoppingItem::manual(command.household_id, command.name, quantity);
        match self.items.add(&item).await {
            Ok(()) => AddItemResponse::Added(item),
            Err(_) => AddItemResponse::Unavailable,
        }
    }
}

// --- Édition --------------------------------------------------------------

/// Command : édite une ligne (champs absents = inchangés).
#[derive(Debug, Clone, Default)]
pub struct UpdateItemCommand {
    /// Foyer propriétaire (scope).
    pub household_id: Option<HouseholdId>,
    /// Ligne visée.
    pub id: Option<ShoppingItemId>,
    /// Nouvel état coché.
    pub checked: Option<bool>,
    /// Nouveau nom.
    pub name: Option<String>,
    /// Nouveau montant (avec `unit`).
    pub amount: Option<f64>,
    /// Nouvelle unité (avec `amount`).
    pub unit: Option<Unit>,
}

/// Résultat d'une édition.
#[derive(Debug)]
pub enum UpdateItemResponse {
    /// Ligne mise à jour.
    Updated(ShoppingItem),
    /// Ligne absente du foyer.
    NotFound,
    /// Entrée invalide.
    Invalid(String),
    /// Panne technique.
    Unavailable,
}

/// Handler de l'édition.
pub struct UpdateItemHandler<'a> {
    items: &'a dyn ShoppingListRepository,
}

impl<'a> UpdateItemHandler<'a> {
    /// Construit le handler.
    #[must_use]
    pub fn new(items: &'a dyn ShoppingListRepository) -> Self {
        Self { items }
    }

    /// Exécute l'édition. Ne renvoie jamais d'erreur.
    pub async fn handle(&self, command: UpdateItemCommand) -> UpdateItemResponse {
        let (Some(household_id), Some(id)) = (command.household_id, command.id) else {
            return UpdateItemResponse::NotFound;
        };
        let existing = match self.items.find(household_id, id).await {
            Ok(Some(item)) => item,
            Ok(None) => return UpdateItemResponse::NotFound,
            Err(_) => return UpdateItemResponse::Unavailable,
        };

        let mut updated = existing;
        if let Some(checked) = command.checked {
            updated.checked = checked;
        }
        if let Some(name) = command.name {
            if name.trim().is_empty() {
                return UpdateItemResponse::Invalid("le nom ne peut pas être vide".to_owned());
            }
            updated.name = name.trim().to_owned();
        }
        // Le montant et l'unité vont de pair : une quantité n'a de sens que
        // complète. À défaut d'unité fournie, on garde celle en place.
        if let Some(amount) = command.amount {
            let unit = command.unit.unwrap_or_else(|| updated.quantity.unit());
            match Quantity::new(amount, unit) {
                Ok(quantity) => updated.quantity = quantity,
                Err(error) => return UpdateItemResponse::Invalid(quantity_error(error)),
            }
        } else if let Some(unit) = command.unit {
            match Quantity::new(updated.quantity.amount(), unit) {
                Ok(quantity) => updated.quantity = quantity,
                Err(error) => return UpdateItemResponse::Invalid(quantity_error(error)),
            }
        }

        match self.items.update(&updated).await {
            Ok(()) => UpdateItemResponse::Updated(updated),
            Err(RepositoryError::NotFound) => UpdateItemResponse::NotFound,
            Err(_) => UpdateItemResponse::Unavailable,
        }
    }
}

// --- Suppression ----------------------------------------------------------

/// Command : supprime une ligne.
#[derive(Debug, Clone)]
pub struct DeleteItemCommand {
    /// Foyer propriétaire (scope).
    pub household_id: HouseholdId,
    /// Ligne visée.
    pub id: ShoppingItemId,
}

/// Résultat d'une suppression.
#[derive(Debug)]
pub enum DeleteItemResponse {
    /// Ligne supprimée.
    Deleted,
    /// Ligne absente du foyer.
    NotFound,
    /// Panne technique.
    Unavailable,
}

/// Handler de la suppression.
pub struct DeleteItemHandler<'a> {
    items: &'a dyn ShoppingListRepository,
}

impl<'a> DeleteItemHandler<'a> {
    /// Construit le handler.
    #[must_use]
    pub fn new(items: &'a dyn ShoppingListRepository) -> Self {
        Self { items }
    }

    /// Exécute la suppression. Ne renvoie jamais d'erreur.
    pub async fn handle(&self, command: DeleteItemCommand) -> DeleteItemResponse {
        match self.items.delete(command.household_id, command.id).await {
            Ok(()) => DeleteItemResponse::Deleted,
            Err(RepositoryError::NotFound) => DeleteItemResponse::NotFound,
            Err(_) => DeleteItemResponse::Unavailable,
        }
    }
}

// --- Vider les cochés -----------------------------------------------------

/// Command : supprime toutes les lignes cochées du foyer.
#[derive(Debug, Clone)]
pub struct ClearCheckedCommand {
    /// Foyer propriétaire (scope).
    pub household_id: HouseholdId,
}

/// Résultat d'un vidage.
#[derive(Debug)]
pub enum ClearCheckedResponse {
    /// Nombre de lignes supprimées (0 si rien n'était coché).
    Cleared(u64),
    /// Panne technique.
    Unavailable,
}

/// Handler du vidage des cochés.
pub struct ClearCheckedHandler<'a> {
    items: &'a dyn ShoppingListRepository,
}

impl<'a> ClearCheckedHandler<'a> {
    /// Construit le handler.
    #[must_use]
    pub fn new(items: &'a dyn ShoppingListRepository) -> Self {
        Self { items }
    }

    /// Exécute le vidage. Ne renvoie jamais d'erreur.
    pub async fn handle(&self, command: ClearCheckedCommand) -> ClearCheckedResponse {
        match self.items.clear_checked(command.household_id).await {
            Ok(count) => ClearCheckedResponse::Cleared(count),
            Err(_) => ClearCheckedResponse::Unavailable,
        }
    }
}

// --- Réordonnancement -----------------------------------------------------

/// Command : fixe l'ordre d'affichage des lignes.
#[derive(Debug, Clone)]
pub struct ReorderCommand {
    /// Foyer propriétaire (scope).
    pub household_id: HouseholdId,
    /// Identifiants dans l'ordre voulu.
    pub ordered_ids: Vec<ShoppingItemId>,
}

/// Résultat d'un réordonnancement.
#[derive(Debug)]
pub enum ReorderResponse {
    /// Ordre appliqué.
    Reordered,
    /// Panne technique.
    Unavailable,
}

/// Handler du réordonnancement.
pub struct ReorderHandler<'a> {
    items: &'a dyn ShoppingListRepository,
}

impl<'a> ReorderHandler<'a> {
    /// Construit le handler.
    #[must_use]
    pub fn new(items: &'a dyn ShoppingListRepository) -> Self {
        Self { items }
    }

    /// Exécute le réordonnancement. Ne renvoie jamais d'erreur.
    pub async fn handle(&self, command: ReorderCommand) -> ReorderResponse {
        match self
            .items
            .reorder(command.household_id, &command.ordered_ids)
            .await
        {
            Ok(()) => ReorderResponse::Reordered,
            Err(_) => ReorderResponse::Unavailable,
        }
    }
}

/// Message lisible pour une quantité refusée par le `kernel`.
fn quantity_error(error: QuantityError) -> String {
    format!("quantité invalide : {error}")
}
