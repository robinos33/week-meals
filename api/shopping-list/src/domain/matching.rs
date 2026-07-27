//! Rapprochement d'un nom d'ingrédient au dictionnaire : primitives **pures**
//! de normalisation et de proximité.
//!
//! Deux recettes ne nomment pas la même chose de la même façon — « Courgettes »,
//! « courgette jaune », « oeufs », « échalotte ». Sans rapprochement, la liste
//! de courses se retrouve avec trois lignes pour un seul légume, et l'ajout
//! manuel ne cumule pas sur la ligne existante. Ce module ramène ces variantes
//! à une **clé commune**, en trois niveaux de tolérance :
//!
//! 1. [`canonical_key`] — casse, accents, ponctuation, mots vides et pluriels
//!    (« Œufs bio » → `oeuf bio`) ;
//! 2. [`core_key`] — en plus, les qualificatifs de courses (couleur, calibre,
//!    préparation) sont retirés (« courgettes jaunes » → `courgette`) ;
//! 3. [`similarity`] — distance de Levenshtein normalisée, pour les fautes de
//!    frappe et les orthographes flottantes (« échalotte » ≈ « échalote »).
//!
//! Les qualificatifs ne sont **pas** de simples mots vides : les retirer d'abord
//! (niveau 2) ferait de « lait de coco » un « lait ». La séparation des niveaux
//! est ce qui permet à [`ReferenceCatalog::resolve`](super::ReferenceCatalog::resolve)
//! de préférer une entrée exacte avant d'élargir.

use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

/// Mots vides du vocabulaire culinaire : ils ne portent aucune information de
/// produit (« gousse **d'**ail », « pomme **de** terre » restent distincts par
/// leurs autres mots).
const STOP_WORDS: &[&str] = &[
    "de", "du", "des", "d", "la", "le", "les", "l", "au", "aux", "a", "en", "et", "un", "une",
];

/// Qualificatifs qui ne changent pas le produit à acheter : couleur, calibre,
/// état, préparation. Retirés au niveau [`core_key`] seulement — « courgette
/// jaune » est une courgette, alors que « lait de coco » n'est pas un lait.
const QUALIFIERS: &[&str] = &[
    "bio",
    "frais",
    "fraiche",
    "surgele",
    "surgelee",
    "congele",
    "congelee",
    "sec",
    "seche",
    "sechee",
    "entier",
    "entiere",
    "gros",
    "grosse",
    "petit",
    "petite",
    "moyen",
    "moyenne",
    "grand",
    "grande",
    "moulu",
    "moulue",
    "rape",
    "rapee",
    "concasse",
    "concassee",
    "emince",
    "emincee",
    "coupe",
    "coupee",
    "cru",
    "crue",
    "cuit",
    "cuite",
    "mur",
    "mure",
    "nouveau",
    "nouvelle",
    "nature",
    "extra",
    "fin",
    "fine",
    "doux",
    "douce",
    "rouge",
    "jaune",
    "vert",
    "verte",
    "blanc",
    "blanche",
    "noir",
    "noire",
    "long",
    "longue",
    "rond",
    "ronde",
    "bien",
    "demi",
    "ecreme",
    "ecremee",
    "pasteurise",
    "pasteurisee",
    "allege",
    "allegee",
];

/// Seuil de similarité au-delà duquel deux noms désignent le même produit.
/// Calibré pour accepter « échalotte » ≈ « échalote » (0,89) et refuser
/// « poire » ≈ « poireau » (0,71).
const SIMILARITY_THRESHOLD: f64 = 0.84;

/// Longueur minimale d'une clé pour tenter une comparaison floue : sur trois
/// lettres, une seule différence suffirait à confondre « ail » et « mail ».
const MIN_FUZZY_LEN: usize = 5;

/// Déplie une chaîne en ASCII minuscule : casse, accents et ligatures.
///
/// La décomposition NFD ne défait pas les ligatures (`œ`, `æ`) : elles sont
/// remplacées avant, sans quoi « œuf » et « oeuf » resteraient étrangers.
fn fold(value: &str) -> String {
    value
        .to_lowercase()
        .replace('œ', "oe")
        .replace('æ', "ae")
        .nfd()
        .filter(|c| !is_combining_mark(*c))
        .collect()
}

/// Ramène un mot au singulier, de façon **cohérente** plutôt que savante :
/// seule compte l'égalité des deux côtés de la comparaison. « ananas » devient
/// « anana » et c'est sans conséquence, puisque l'entrée du dictionnaire subit
/// le même sort.
fn singular(token: &str) -> String {
    if token.len() > 4 && token.ends_with("aux") {
        // chevaux → cheval, bocaux → bocal.
        return format!("{}al", &token[..token.len() - 3]);
    }
    if token.len() > 3 && (token.ends_with('s') || token.ends_with('x')) {
        return token[..token.len() - 1].to_owned();
    }
    token.to_owned()
}

/// Découpe un nom en mots normalisés : dépliés, sans ponctuation, sans mots
/// vides, au singulier. C'est la brique commune des trois niveaux.
#[must_use]
pub fn tokens(name: &str) -> Vec<String> {
    fold(name)
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .filter(|word| !STOP_WORDS.contains(word))
        .map(singular)
        .filter(|word| !word.is_empty())
        .collect()
}

/// `true` si le mot est un qualificatif de courses (cf. [`QUALIFIERS`]).
///
/// La comparaison accepte la forme singularisée de la table : les jetons sont
/// passés au singulier avant d'arriver ici (« gros » y devient « gro »), et on
/// préfère garder la table lisible plutôt que d'y écrire des moignons.
#[must_use]
pub fn is_qualifier(token: &str) -> bool {
    QUALIFIERS
        .iter()
        .any(|qualifier| token == *qualifier || token == singular(qualifier))
}

/// Clé de rapprochement d'un nom : mots normalisés joints par une espace.
/// « Œufs de poule » et « oeuf poule » partagent la même.
#[must_use]
pub fn canonical_key(name: &str) -> String {
    tokens(name).join(" ")
}

/// Clé « produit », qualificatifs retirés : « courgettes jaunes » → `courgette`.
///
/// Vide si le nom n'était **que** des qualificatifs (« bio ») : l'appelant
/// retombe alors sur [`canonical_key`], on ne rapproche pas sur du vide.
#[must_use]
pub fn core_key(name: &str) -> String {
    tokens(name)
        .into_iter()
        .filter(|token| !is_qualifier(token))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Proximité de deux clés, entre 0 (rien en commun) et 1 (identiques) :
/// distance de Levenshtein rapportée à la longueur de la plus longue.
#[must_use]
pub fn similarity(left: &str, right: &str) -> f64 {
    if left == right {
        return 1.0;
    }
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    let longest = left.len().max(right.len());
    if longest == 0 {
        return 1.0;
    }
    let distance = levenshtein(&left, &right);
    1.0 - distance as f64 / longest as f64
}

/// `true` si les deux clés sont assez proches pour désigner le même produit.
/// Les clés trop courtes sont exclues : sur trois lettres, la distance de
/// Levenshtein ne discrimine plus rien.
#[must_use]
pub fn is_close(left: &str, right: &str) -> bool {
    left.len() >= MIN_FUZZY_LEN
        && right.len() >= MIN_FUZZY_LEN
        && similarity(left, right) >= SIMILARITY_THRESHOLD
}

/// Distance d'édition, en mémoire O(min(n, m)) : deux lignes de la matrice
/// suffisent, on ne reconstruit jamais le chemin.
fn levenshtein(left: &[char], right: &[char]) -> usize {
    if left.is_empty() {
        return right.len();
    }
    if right.is_empty() {
        return left.len();
    }

    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut current = vec![0; right.len() + 1];

    for (i, left_char) in left.iter().enumerate() {
        current[0] = i + 1;
        for (j, right_char) in right.iter().enumerate() {
            let substitution = usize::from(left_char != right_char);
            current[j + 1] = (previous[j] + substitution)
                .min(previous[j + 1] + 1) // suppression
                .min(current[j] + 1); // insertion
        }
        std::mem::swap(&mut previous, &mut current);
    }
    previous[right.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_key_folds_case_accents_and_ligatures() {
        assert_eq!(canonical_key("  Œufs  "), "oeuf");
        assert_eq!(canonical_key("oeufs"), canonical_key("Œufs"));
        assert_eq!(canonical_key("Échalotes"), "echalote");
    }

    #[test]
    fn canonical_key_drops_stop_words_and_punctuation() {
        assert_eq!(canonical_key("gousse d'ail"), "gousse ail");
        assert_eq!(canonical_key("pommes de terre"), "pomme terre");
    }

    #[test]
    fn canonical_key_singularises_regular_plurals() {
        assert_eq!(canonical_key("tomates"), canonical_key("tomate"));
        assert_eq!(canonical_key("choux"), canonical_key("chou"));
        assert_eq!(canonical_key("bocaux"), "bocal");
    }

    #[test]
    fn short_words_are_left_alone() {
        // « riz » ne doit pas devenir « ri », ni « os » perdre son pluriel.
        assert_eq!(canonical_key("riz"), "riz");
        assert_eq!(canonical_key("ail"), "ail");
    }

    #[test]
    fn core_key_strips_shopping_qualifiers() {
        assert_eq!(core_key("courgettes jaunes"), "courgette");
        assert_eq!(core_key("Œufs bio"), "oeuf");
        assert_eq!(core_key("carottes nouvelles"), "carotte");
    }

    #[test]
    fn core_key_keeps_words_that_change_the_product() {
        // « coco » n'est pas un qualificatif : le lait de coco reste distinct.
        assert_eq!(core_key("lait de coco"), "lait coco");
        assert_eq!(core_key("pomme de terre"), "pomme terre");
    }

    #[test]
    fn core_key_is_empty_when_only_qualifiers_remain() {
        assert_eq!(core_key("bio"), "");
    }

    #[test]
    fn similarity_is_one_for_identical_keys() {
        assert!((similarity("courgette", "courgette") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn close_spellings_are_matched() {
        assert!(is_close("echalotte", "echalote"), "une lettre en trop");
        assert!(is_close("courgete", "courgette"), "une lettre manquante");
    }

    #[test]
    fn different_products_are_not_matched() {
        assert!(!is_close("poire", "poireau"), "deux produits distincts");
        assert!(!is_close("citron", "citrouille"));
        assert!(!is_close("lait", "ail"), "clés trop courtes");
    }

    #[test]
    fn empty_key_matches_nothing() {
        assert!(!is_close("", "courgette"));
    }
}
