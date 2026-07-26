//! Use cases d'écriture de la liste de courses : génération depuis le
//! calendrier, ajout manuel, édition, suppression, vidage des cochés.

use std::collections::HashSet;

use chrono::NaiveDate;
use kernel::{HouseholdId, Quantity, QuantityError, RepositoryError, ShoppingItemId, Unit};

use crate::domain::reference::normalize_name;
use crate::domain::{
    aggregate_purchases, CookedCountRecorder, PlannedIngredientsSource, ReferenceRepository,
    ShoppingItem, ShoppingListRepository,
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
    cooked: &'a dyn CookedCountRecorder,
}

impl<'a> GenerateListHandler<'a> {
    /// Construit le handler.
    #[must_use]
    pub fn new(
        items: &'a dyn ShoppingListRepository,
        references: &'a dyn ReferenceRepository,
        planned: &'a dyn PlannedIngredientsSource,
        cooked: &'a dyn CookedCountRecorder,
    ) -> Self {
        Self {
            items,
            references,
            planned,
            cooked,
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

        // Générer vaut engagement (#58) : on incrémente le compteur « cuisiné X
        // fois » des recettes de la plage. Fait **après** le remplacement des
        // lignes, et sur une garde par créneau (`counted_at`) : si l'appel
        // échoue, les cases restent non comptées et seront rattrapées à la
        // prochaine génération — jamais de double comptage, jamais de perte.
        if self
            .cooked
            .record_cooked(command.household_id, command.from, command.to)
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
    ///
    /// Additionne plutôt que de dupliquer : si un article **non coché** du même
    /// ingrédient (nom normalisé) et de **même dimension** existe déjà, la
    /// quantité est cumulée sur cette ligne (dans son unité) — jamais deux fois
    /// le même ingrédient dans la liste. À défaut, nouvelle ligne manuelle.
    pub async fn handle(&self, command: AddItemCommand) -> AddItemResponse {
        if command.name.trim().is_empty() {
            return AddItemResponse::Invalid("le nom ne peut pas être vide".to_owned());
        }
        let quantity = match Quantity::new(command.amount, command.unit) {
            Ok(quantity) => quantity,
            Err(error) => return AddItemResponse::Invalid(quantity_error(error)),
        };

        let existing = match self.items.list(command.household_id).await {
            Ok(items) => items,
            Err(_) => return AddItemResponse::Unavailable,
        };
        let key = normalize_name(&command.name);
        let mergeable = existing.into_iter().find(|item| {
            !item.checked
                && item.quantity.dimension() == quantity.dimension()
                && normalize_name(&item.name) == key
        });

        if let Some(target) = mergeable {
            let merged = merge(target, quantity);
            return match self.items.update(&merged).await {
                Ok(()) => AddItemResponse::Added(merged),
                Err(_) => AddItemResponse::Unavailable,
            };
        }

        let item = ShoppingItem::manual(command.household_id, command.name, quantity);
        match self.items.add(&item).await {
            Ok(()) => AddItemResponse::Added(item),
            Err(_) => AddItemResponse::Unavailable,
        }
    }
}

/// Cumule `added` sur la ligne `target` (même dimension garantie) et renvoie la
/// ligne mise à jour.
///
/// La somme se fait dans l'unité de base de la dimension puis se ré-exprime dans
/// l'unité de la ligne cible (500 g + 0,5 kg → 1000 g ; 1 kg + 500 g → 1,5 kg).
/// Le résultat, somme de deux montants strictement positifs finis, est toujours
/// une quantité valide.
fn merge(mut target: ShoppingItem, added: Quantity) -> ShoppingItem {
    let unit = target.quantity.unit();
    let summed = (target.quantity.in_base() + added.in_base()) / unit.base_factor();
    target.quantity = Quantity::new(summed, unit)
        .expect("la somme de deux quantités positives finies est valide");
    target
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ShoppingItem;
    use crate::testing::InMemoryShoppingList;
    use kernel::ShoppingItemId;

    fn household() -> HouseholdId {
        HouseholdId::new()
    }

    fn generated(household_id: HouseholdId, name: &str, amount: f64, unit: Unit) -> ShoppingItem {
        ShoppingItem {
            id: ShoppingItemId::new(),
            household_id,
            name: name.to_owned(),
            quantity: Quantity::new(amount, unit).unwrap(),
            category: Some("legumes".to_owned()),
            checked: false,
            generated: true,
            position: 0,
        }
    }

    async fn add(repo: &InMemoryShoppingList, h: HouseholdId, name: &str, amount: f64, unit: Unit) {
        let response = AddItemHandler::new(repo)
            .handle(AddItemCommand {
                household_id: h,
                name: name.to_owned(),
                amount,
                unit,
            })
            .await;
        assert!(
            matches!(response, AddItemResponse::Added(_)),
            "ajout accepté"
        );
    }

    #[tokio::test]
    async fn adding_a_new_ingredient_creates_a_line() {
        let repo = InMemoryShoppingList::default();
        let h = household();
        add(&repo, h, "courgette", 3.0, Unit::Piece).await;
        let items = repo.list(h).await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].quantity, Quantity::new(3.0, Unit::Piece).unwrap());
    }

    #[tokio::test]
    async fn adding_an_existing_ingredient_sums_quantities() {
        let repo = InMemoryShoppingList::default();
        let h = household();
        add(&repo, h, "courgette", 3.0, Unit::Piece).await;
        add(&repo, h, "courgette", 2.0, Unit::Piece).await;

        let items = repo.list(h).await.unwrap();
        assert_eq!(items.len(), 1, "une seule ligne, pas de doublon");
        assert_eq!(items[0].quantity, Quantity::new(5.0, Unit::Piece).unwrap());
    }

    #[tokio::test]
    async fn sum_converts_into_the_existing_line_unit() {
        // 500 g déjà présents + 0,5 kg ajoutés → 1000 g (unité de la ligne).
        let repo = InMemoryShoppingList::default();
        let h = household();
        add(&repo, h, "farine", 500.0, Unit::G).await;
        add(&repo, h, "farine", 0.5, Unit::Kg).await;

        let items = repo.list(h).await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].quantity, Quantity::new(1000.0, Unit::G).unwrap());
    }

    #[tokio::test]
    async fn name_match_ignores_case_and_whitespace() {
        let repo = InMemoryShoppingList::default();
        let h = household();
        add(&repo, h, "Courgette", 3.0, Unit::Piece).await;
        add(&repo, h, "  courgette ", 2.0, Unit::Piece).await;

        let items = repo.list(h).await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].quantity, Quantity::new(5.0, Unit::Piece).unwrap());
    }

    #[tokio::test]
    async fn manual_add_merges_into_a_generated_line() {
        // Le doublon le plus courant : un ajout manuel sur un ingrédient déjà
        // généré depuis le calendrier. On cumule sur la ligne générée.
        let h = household();
        let repo = InMemoryShoppingList::with(vec![generated(h, "courgette", 4.0, Unit::Piece)]);
        add(&repo, h, "courgette", 2.0, Unit::Piece).await;

        let items = repo.list(h).await.unwrap();
        assert_eq!(items.len(), 1, "fusion sur la ligne générée");
        assert_eq!(items[0].quantity, Quantity::new(6.0, Unit::Piece).unwrap());
        assert!(items[0].generated, "la ligne reste générée");
    }

    #[tokio::test]
    async fn different_dimensions_stay_separate() {
        let repo = InMemoryShoppingList::default();
        let h = household();
        add(&repo, h, "sirop", 3.0, Unit::Piece).await;
        add(&repo, h, "sirop", 200.0, Unit::Ml).await;

        let items = repo.list(h).await.unwrap();
        assert_eq!(items.len(), 2, "pièces et mL ne se cumulent pas");
    }

    #[tokio::test]
    async fn checked_line_is_not_merged_into() {
        // Un article déjà coché (acheté) n'absorbe pas un réajout : nouvelle
        // ligne, à cocher à son tour.
        let mut done = generated(household(), "courgette", 4.0, Unit::Piece);
        done.checked = true;
        let h = done.household_id;
        let repo = InMemoryShoppingList::with(vec![done]);
        add(&repo, h, "courgette", 2.0, Unit::Piece).await;

        let items = repo.list(h).await.unwrap();
        assert_eq!(
            items.len(),
            2,
            "le coché reste, une nouvelle ligne est créée"
        );
    }

    #[tokio::test]
    async fn empty_name_is_rejected() {
        let repo = InMemoryShoppingList::default();
        let response = AddItemHandler::new(&repo)
            .handle(AddItemCommand {
                household_id: household(),
                name: "   ".to_owned(),
                amount: 1.0,
                unit: Unit::Piece,
            })
            .await;
        assert!(matches!(response, AddItemResponse::Invalid(_)));
    }
}
