//! Les **magasins** d'un foyer : un nom et l'ordre dans lequel on en parcourt
//! les rayons. Aucune dépendance à SQLx/Axum (règle de convention).
//!
//! Un magasin ne décrit pas un catalogue de produits : il décrit un **trajet**.
//! Chez l'un, les surgelés sont à l'entrée ; chez l'autre, juste avant les
//! caisses. Ranger les rayons dans l'ordre de visite, magasin par magasin, est
//! ce qui permet de trier la liste de courses pour ne faire qu'un aller simple
//! (cf. [`aisle`](super::aisle) pour le catalogue des rayons).
//!
//! Un magasin porte **tous** les rayons du catalogue, jamais un sous-ensemble :
//! ce qui n'est pas acheté ne s'affiche pas de toute façon (un rayon sans
//! article ne produit pas de section), tandis qu'un rayon oublié laisserait des
//! articles sans place. L'ordre stocké est donc toujours complet et sans
//! doublon — [`Store::reorder`] s'en porte garant.

use kernel::{HouseholdId, RepositoryError, StoreId};

use super::aisle;

/// Longueur maximale du nom d'un magasin (l'écran en affiche une ligne).
const MAX_NAME_LEN: usize = 60;

/// Ce qu'un magasin refuse.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    /// Nom vide (après trim).
    #[error("le nom du magasin est vide")]
    EmptyName,
    /// Nom trop long.
    #[error("le nom du magasin dépasse {MAX_NAME_LEN} caractères")]
    NameTooLong,
}

/// Un magasin du foyer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Store {
    /// Identifiant du magasin.
    pub id: StoreId,
    /// Foyer propriétaire (scope).
    pub household_id: HouseholdId,
    /// Nom affiché (« Super U des Halles »).
    pub name: String,
    /// Slugs des rayons, dans l'ordre de visite. Toujours complet et sans
    /// doublon (cf. [`Store::reorder`]).
    pub aisles: Vec<String>,
    /// Ordre d'affichage du magasin dans la liste du foyer.
    pub position: i32,
}

impl Store {
    /// Crée un magasin dans l'ordre de rayons par défaut.
    ///
    /// # Errors
    /// [`StoreError::EmptyName`] / [`StoreError::NameTooLong`] si le nom est
    /// hors format.
    pub fn new(household_id: HouseholdId, name: &str) -> Result<Self, StoreError> {
        Ok(Self {
            id: StoreId::new(),
            household_id,
            name: valid_name(name)?,
            aisles: aisle::default_order(),
            position: 0,
        })
    }

    /// Renomme le magasin.
    ///
    /// # Errors
    /// Voir [`Store::new`].
    pub fn rename(&mut self, name: &str) -> Result<(), StoreError> {
        self.name = valid_name(name)?;
        Ok(())
    }

    /// Réordonne les rayons d'après `order`, en **complétant** ce qui manque.
    ///
    /// Le client n'a pas à envoyer une liste parfaite : les slugs inconnus et
    /// les doublons sont écartés, puis les rayons non cités sont ajoutés à la
    /// suite dans l'ordre par défaut. Deux conséquences voulues : un ordre
    /// partiel (ou une version du client en retard d'un rayon) ne fait rien
    /// perdre, et un rayon ajouté par une future version de l'app se retrouve
    /// simplement en fin de parcours plutôt que nulle part.
    pub fn reorder(&mut self, order: &[String]) {
        let mut ordered: Vec<String> = Vec::with_capacity(aisle::AISLES.len());
        for slug in order {
            let slug = slug.trim();
            if aisle::is_known(slug) && !ordered.iter().any(|kept| kept == slug) {
                ordered.push(slug.to_owned());
            }
        }
        for slug in aisle::default_order() {
            if !ordered.contains(&slug) {
                ordered.push(slug);
            }
        }
        self.aisles = ordered;
    }
}

/// Valide et normalise un nom de magasin.
fn valid_name(name: &str) -> Result<String, StoreError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(StoreError::EmptyName);
    }
    if name.chars().count() > MAX_NAME_LEN {
        return Err(StoreError::NameTooLong);
    }
    Ok(name.to_owned())
}

/// Port de persistance des magasins. Tout est scopé à un [`HouseholdId`].
#[async_trait::async_trait]
pub trait StoreRepository: Send + Sync {
    /// Magasins du foyer, ordonnés par position puis par nom.
    async fn list(&self, household_id: HouseholdId) -> Result<Vec<Store>, RepositoryError>;

    /// Récupère un magasin du foyer.
    async fn find(
        &self,
        household_id: HouseholdId,
        id: StoreId,
    ) -> Result<Option<Store>, RepositoryError>;

    /// Enregistre un magasin (création ou mise à jour), rayons compris.
    async fn save(&self, store: &Store) -> Result<(), RepositoryError>;

    /// Supprime un magasin.
    ///
    /// # Errors
    /// [`RepositoryError::NotFound`] si le magasin n'existe pas dans le foyer.
    async fn delete(&self, household_id: HouseholdId, id: StoreId) -> Result<(), RepositoryError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        Store::new(HouseholdId::new(), "  Super U  ").unwrap()
    }

    #[test]
    fn trims_and_rejects_blank_names() {
        assert_eq!(store().name, "Super U");
        assert_eq!(
            Store::new(HouseholdId::new(), "   ").unwrap_err(),
            StoreError::EmptyName
        );
        assert_eq!(
            Store::new(HouseholdId::new(), &"x".repeat(61)).unwrap_err(),
            StoreError::NameTooLong
        );
    }

    #[test]
    fn a_new_store_follows_the_default_aisle_order() {
        assert_eq!(store().aisles, aisle::default_order());
    }

    #[test]
    fn reorder_puts_the_given_aisles_first() {
        let mut store = store();
        store.reorder(&["surgeles".to_owned(), "fruits".to_owned()]);
        assert_eq!(store.aisles[0], "surgeles");
        assert_eq!(store.aisles[1], "fruits");
    }

    #[test]
    fn reorder_completes_a_partial_order() {
        // Rien ne se perd : les rayons non cités suivent, dans l'ordre par
        // défaut — un client en retard d'une version ne vide pas le magasin.
        let mut store = store();
        store.reorder(&["boissons".to_owned()]);
        assert_eq!(store.aisles.len(), aisle::AISLES.len());
        assert_eq!(store.aisles[0], "boissons");
        assert_eq!(store.aisles[1], "fruits");
    }

    #[test]
    fn reorder_drops_unknown_slugs_and_duplicates() {
        let mut store = store();
        store.reorder(&[
            "legumes".to_owned(),
            "epicerie".to_owned(), // ancien slug, scindé depuis
            "legumes".to_owned(),
        ]);
        assert_eq!(store.aisles[0], "legumes");
        assert_eq!(store.aisles.len(), aisle::AISLES.len());
        assert_eq!(
            store
                .aisles
                .iter()
                .filter(|slug| *slug == "legumes")
                .count(),
            1
        );
    }

    #[test]
    fn rename_validates_like_creation() {
        let mut store = store();
        store.rename(" Lidl ").unwrap();
        assert_eq!(store.name, "Lidl");
        assert_eq!(store.rename("  ").unwrap_err(), StoreError::EmptyName);
    }
}
