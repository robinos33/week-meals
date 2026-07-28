//! Le catalogue des **rayons** : le vocabulaire de classement du dictionnaire
//! d'ingrédients, et la matière première de l'ordre de visite d'un magasin.
//!
//! Un rayon est identifié par un `slug` stable (`epicerie-salee`) — c'est lui
//! qui est stocké, aussi bien dans `ingredient_reference.category` que dans
//! l'ordre des rayons d'un magasin. Le libellé n'est qu'un affichage.
//!
//! Le catalogue est **fermé** : `data/ingredients.yaml` ne peut ranger un
//! ingrédient que dans un rayon d'ici (un test le vérifie sur le fichier
//! versionné). C'est ce qui garantit qu'un magasin sait ordonner tout ce que la
//! liste peut contenir — un rayon inconnu ne se rangerait nulle part.
//!
//! L'ordre de déclaration est l'**ordre par défaut** : celui d'un supermarché
//! type, du frais à l'entrée jusqu'au bazar au fond. Chaque foyer le rectifie
//! magasin par magasin (cf. [`store`](super::store)) — c'est tout l'intérêt.

/// Un rayon du catalogue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Aisle {
    /// Identifiant stable, stocké en base (`legumes`, `epicerie-salee`).
    pub slug: &'static str,
    /// Libellé affiché (français).
    pub label: &'static str,
}

/// Construit une entrée du catalogue.
const fn aisle(slug: &'static str, label: &'static str) -> Aisle {
    Aisle { slug, label }
}

/// Tous les rayons, dans l'ordre de visite **par défaut**.
///
/// Ce qui se garde au frais d'abord (on finit par le surgelé), l'épicerie
/// ensuite, le non-alimentaire pour finir. Le découpage suit ce qu'on trouve
/// vraiment en magasin : l'« épicerie » d'un supermarché n'est pas une allée
/// mais quatre — salée, sucrée, condiments, boissons —, et les séparer est ce
/// qui évite les allers-retours une fois la liste triée.
pub const AISLES: &[Aisle] = &[
    aisle("fruits", "Fruits"),
    aisle("legumes", "Légumes"),
    aisle("boulangerie", "Boulangerie"),
    aisle("boucherie", "Boucherie"),
    aisle("charcuterie", "Charcuterie & traiteur"),
    aisle("poissonnerie", "Poissonnerie"),
    aisle("cremerie", "Crèmerie"),
    aisle("surgeles", "Surgelés"),
    aisle("epicerie-salee", "Épicerie salée"),
    aisle("epicerie-sucree", "Épicerie sucrée"),
    aisle("condiments", "Condiments & épices"),
    aisle("boissons", "Boissons"),
    aisle("hygiene", "Hygiène & beauté"),
    aisle("entretien", "Entretien & ménage"),
    aisle("maison", "Maison & bazar"),
    aisle("bebe", "Bébé"),
    aisle("animaux", "Animaux"),
];

/// `true` si le slug est un rayon du catalogue.
#[must_use]
pub fn is_known(slug: &str) -> bool {
    AISLES.iter().any(|aisle| aisle.slug == slug)
}

/// Libellé d'un rayon, ou `None` s'il est inconnu.
#[must_use]
pub fn label(slug: &str) -> Option<&'static str> {
    AISLES
        .iter()
        .find(|aisle| aisle.slug == slug)
        .map(|aisle| aisle.label)
}

/// Rang d'un rayon dans l'ordre par défaut, ou `None` s'il est inconnu.
#[must_use]
pub fn default_rank(slug: &str) -> Option<usize> {
    AISLES.iter().position(|aisle| aisle.slug == slug)
}

/// L'ordre par défaut, en clair.
#[must_use]
pub fn default_order() -> Vec<String> {
    AISLES.iter().map(|aisle| aisle.slug.to_owned()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_are_unique() {
        let mut slugs: Vec<&str> = AISLES.iter().map(|aisle| aisle.slug).collect();
        slugs.sort_unstable();
        let count = slugs.len();
        slugs.dedup();
        assert_eq!(slugs.len(), count, "un slug de rayon est déclaré deux fois");
    }

    #[test]
    fn known_aisles_have_a_label_and_a_rank() {
        assert!(is_known("legumes"));
        assert_eq!(label("legumes"), Some("Légumes"));
        assert_eq!(default_rank("fruits"), Some(0));
    }

    #[test]
    fn unknown_aisles_are_rejected() {
        // « epicerie » a été scindé en salée / sucrée / condiments : l'ancien
        // slug ne doit plus passer pour un rayon connu.
        assert!(!is_known("epicerie"));
        assert_eq!(label("inconnu"), None);
        assert_eq!(default_rank("inconnu"), None);
    }

    #[test]
    fn default_order_lists_every_aisle_once() {
        assert_eq!(default_order().len(), AISLES.len());
        assert_eq!(default_order()[0], "fruits");
    }
}
