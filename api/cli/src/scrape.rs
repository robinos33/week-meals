//! Scraping d'une page de recette vers le format YAML de seed (#26).
//!
//! Stratégie : lire le **JSON-LD schema.org** (`<script type="application/ld+json">`
//! avec `@type: Recipe`) que publient la plupart des sites de cuisine, plutôt
//! que des sélecteurs HTML propres à chaque site — c'est le seul contrat
//! réellement partagé entre eux.
//!
//! Le résultat est un **brouillon à relire** : les quantités des sites sont du
//! texte libre (« 2 c. à soupe d'huile »), leur découpage en `quantity`/`unit`
//! est heuristique. Le flux prévu est donc `scrape > fichier.yaml`, on relit,
//! puis `import`.

use anyhow::{bail, Context, Result};
use kernel::Unit;
use regex::Regex;
use serde_json::Value;

use crate::recipe_yaml::{IngredientYaml, RecipeYaml};

/// Récupère la page et en extrait une recette au format YAML.
///
/// # Errors
/// Si la page est injoignable, ou si elle ne publie pas de recette JSON-LD.
pub async fn scrape(url: &str) -> Result<RecipeYaml> {
    let html = fetch(url).await?;
    let recipe = find_recipe(&html).with_context(|| {
        format!("aucune recette schema.org (JSON-LD) trouvée sur {url} — site non supporté")
    })?;
    Ok(map_recipe(&recipe))
}

/// Télécharge la page en se présentant comme un navigateur (certains sites
/// refusent un User-Agent vide).
async fn fetch(url: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .user_agent(
            "Mozilla/5.0 (compatible; weekmeals/0.1; +https://github.com/robinos33/week-meals)",
        )
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .context("construction du client HTTP")?;

    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("requête vers {url}"))?;

    if !response.status().is_success() {
        bail!("{url} a répondu {}", response.status());
    }
    response.text().await.context("lecture de la réponse")
}

// --- Extraction JSON-LD ---------------------------------------------------

/// Cherche le premier objet `Recipe` parmi les blocs JSON-LD de la page.
fn find_recipe(html: &str) -> Option<Value> {
    // `(?is)` : insensible à la casse et `.` couvre les retours à la ligne.
    //
    // Le `/` et le `+` du type MIME sont acceptés échappés en entités HTML :
    // Marmiton, par exemple, sert `type="application&#x2F;ld&#x2B;json"`, ce
    // qu'un motif littéral raterait.
    let script = Regex::new(
        r#"(?is)<script[^>]*type\s*=\s*["']application(?:/|&#x2f;|&#47;)ld(?:\+|&#x2b;|&#43;)json["'][^>]*>(.*?)</script>"#,
    )
    .ok()?;

    for capture in script.captures_iter(html) {
        let raw = capture.get(1)?.as_str();
        let Ok(value) = serde_json::from_str::<Value>(raw.trim()) else {
            continue; // un bloc invalide ne doit pas condamner les suivants
        };
        if let Some(recipe) = find_recipe_in(&value) {
            return Some(recipe);
        }
    }
    None
}

/// Descend dans un document JSON-LD (objet, tableau ou `@graph`) à la recherche
/// d'un nœud `Recipe`.
fn find_recipe_in(value: &Value) -> Option<Value> {
    match value {
        Value::Array(items) => items.iter().find_map(find_recipe_in),
        Value::Object(object) => {
            if is_recipe(value) {
                return Some(value.clone());
            }
            object.get("@graph").and_then(find_recipe_in)
        }
        _ => None,
    }
}

/// `@type` peut être une chaîne (`"Recipe"`) ou un tableau (`["Recipe", ...]`).
fn is_recipe(value: &Value) -> bool {
    match value.get("@type") {
        Some(Value::String(kind)) => kind.eq_ignore_ascii_case("recipe"),
        Some(Value::Array(kinds)) => kinds
            .iter()
            .filter_map(Value::as_str)
            .any(|kind| kind.eq_ignore_ascii_case("recipe")),
        _ => false,
    }
}

// --- Mapping vers le format de seed ---------------------------------------

/// Projette un nœud `Recipe` JSON-LD vers le YAML de seed.
fn map_recipe(recipe: &Value) -> RecipeYaml {
    RecipeYaml {
        title: recipe
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Sans titre")
            .trim()
            .to_owned(),
        prep_time_min: recipe
            .get("prepTime")
            .and_then(Value::as_str)
            .and_then(parse_duration),
        cook_time_min: recipe
            .get("cookTime")
            .and_then(Value::as_str)
            .and_then(parse_duration),
        photo: extract_image(recipe.get("image")),
        ingredients: recipe
            .get("recipeIngredient")
            .and_then(Value::as_array)
            .map(|lines| {
                lines
                    .iter()
                    .filter_map(Value::as_str)
                    .map(parse_ingredient)
                    .collect()
            })
            .unwrap_or_default(),
        steps: extract_steps(recipe.get("recipeInstructions")),
    }
}

/// `image` peut être une URL, un tableau d'URLs, ou un objet `ImageObject`.
fn extract_image(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(url) => Some(url.clone()),
        Value::Array(items) => items.first().and_then(|first| extract_image(Some(first))),
        Value::Object(object) => object.get("url").and_then(Value::as_str).map(str::to_owned),
        _ => None,
    }
}

/// `recipeInstructions` peut être un texte, un tableau de textes, de
/// `HowToStep`, ou de `HowToSection` (qui contient lui-même des étapes).
fn extract_steps(value: Option<&Value>) -> Vec<String> {
    fn push_steps(value: &Value, steps: &mut Vec<String>) {
        match value {
            Value::String(text) => {
                let text = text.trim();
                if !text.is_empty() {
                    steps.push(text.to_owned());
                }
            }
            Value::Array(items) => items.iter().for_each(|item| push_steps(item, steps)),
            Value::Object(object) => {
                // Une section porte ses étapes dans `itemListElement`.
                if let Some(children) = object.get("itemListElement") {
                    push_steps(children, steps);
                } else if let Some(text) = object.get("text").or_else(|| object.get("name")) {
                    push_steps(text, steps);
                }
            }
            _ => {}
        }
    }

    let mut steps = Vec::new();
    if let Some(value) = value {
        push_steps(value, &mut steps);
    }
    steps
}

/// Convertit une durée ISO 8601 (`PT1H30M`) en minutes.
fn parse_duration(raw: &str) -> Option<u32> {
    let raw = raw.trim().to_uppercase();
    let rest = raw.strip_prefix("PT")?;
    let mut minutes = 0u32;
    let mut number = String::new();
    for ch in rest.chars() {
        if ch.is_ascii_digit() {
            number.push(ch);
            continue;
        }
        let value: u32 = number.parse().ok()?;
        number.clear();
        match ch {
            'H' => minutes += value * 60,
            'M' => minutes += value,
            'S' => {} // on ignore les secondes
            _ => return None,
        }
    }
    (minutes > 0).then_some(minutes)
}

// --- Découpage d'une ligne d'ingrédient -----------------------------------

/// Unités reconnues, du libellé le plus long au plus court (le premier qui
/// colle gagne), avec le facteur vers l'unité du `kernel`.
const UNITS: &[(&str, Unit, f64)] = &[
    ("cuillères à soupe", Unit::Ml, 15.0),
    ("cuillère à soupe", Unit::Ml, 15.0),
    ("cuillères à café", Unit::Ml, 5.0),
    ("cuillère à café", Unit::Ml, 5.0),
    ("c. à soupe", Unit::Ml, 15.0),
    ("c.à.s", Unit::Ml, 15.0),
    ("c. à café", Unit::Ml, 5.0),
    ("c.à.c", Unit::Ml, 5.0),
    ("kilogrammes", Unit::Kg, 1.0),
    ("kilogramme", Unit::Kg, 1.0),
    ("millilitres", Unit::Ml, 1.0),
    ("millilitre", Unit::Ml, 1.0),
    ("centilitres", Unit::Ml, 10.0),
    ("centilitre", Unit::Ml, 10.0),
    ("grammes", Unit::G, 1.0),
    ("gramme", Unit::G, 1.0),
    ("litres", Unit::L, 1.0),
    ("litre", Unit::L, 1.0),
    ("kilos", Unit::Kg, 1.0),
    ("kilo", Unit::Kg, 1.0),
    ("pincées", Unit::Piece, 1.0),
    ("pincée", Unit::Piece, 1.0),
    ("kg", Unit::Kg, 1.0),
    ("mg", Unit::G, 0.001),
    ("ml", Unit::Ml, 1.0),
    ("cl", Unit::Ml, 10.0),
    ("dl", Unit::Ml, 100.0),
    ("gr", Unit::G, 1.0),
    ("g", Unit::G, 1.0),
    ("l", Unit::L, 1.0),
];

/// Découpe une ligne libre en quantité / unité / nom.
///
/// Heuristique assumée : à défaut de quantité lisible, la ligne devient
/// « 1 pièce » du texte entier — le YAML est de toute façon relu à la main.
fn parse_ingredient(line: &str) -> IngredientYaml {
    let cleaned = line.replace('\u{a0}', " ");
    let cleaned = cleaned.trim();

    let Some((amount, rest)) = parse_amount(cleaned) else {
        return IngredientYaml {
            name: cleaned.to_owned(),
            quantity: 1.0,
            unit: Unit::Piece,
        };
    };

    let (unit, factor, rest) = match_unit(rest);
    let name = strip_article(rest);
    // Un nom vide (« 3 œufs » où tout a été consommé) : on retombe sur la
    // ligne d'origine plutôt que de produire une entrée anonyme.
    let name = if name.is_empty() { cleaned } else { name };

    IngredientYaml {
        name: name.to_owned(),
        quantity: (amount * factor).max(f64::MIN_POSITIVE),
        unit,
    }
}

/// Lit une quantité en tête : `600`, `1,5`, `1/2`, `1 1/2`.
fn parse_amount(input: &str) -> Option<(f64, &str)> {
    let number =
        Regex::new(r"^(\d+)\s+(\d+)\s*/\s*(\d+)|^(\d+)\s*/\s*(\d+)|^(\d+(?:[.,]\d+)?)").ok()?;
    let capture = number.captures(input)?;

    let amount = if let (Some(whole), Some(num), Some(den)) =
        (capture.get(1), capture.get(2), capture.get(3))
    {
        parse_f64(whole.as_str())? + parse_f64(num.as_str())? / parse_f64(den.as_str())?
    } else if let (Some(num), Some(den)) = (capture.get(4), capture.get(5)) {
        parse_f64(num.as_str())? / parse_f64(den.as_str())?
    } else {
        parse_f64(capture.get(6)?.as_str())?
    };

    let rest = input[capture.get(0)?.end()..].trim_start();
    (amount > 0.0).then_some((amount, rest))
}

/// Lit un nombre en tolérant la virgule décimale française.
fn parse_f64(raw: &str) -> Option<f64> {
    raw.replace(',', ".").parse().ok()
}

/// Détache l'unité en tête, si elle est reconnue.
fn match_unit(input: &str) -> (Unit, f64, &str) {
    let lower = input.to_lowercase();
    for (label, unit, factor) in UNITS {
        let Some(rest) = lower.strip_prefix(label) else {
            continue;
        };
        // L'unité doit être un mot entier : « gousse » ne commence pas par « g ».
        if rest
            .chars()
            .next()
            .is_some_and(|ch| ch.is_alphanumeric() || ch == '\'')
        {
            continue;
        }
        return (*unit, *factor, input[label.len()..].trim_start());
    }
    (Unit::Piece, 1.0, input)
}

/// Retire l'article qui suit l'unité (« de », « d' », « du », « des »).
fn strip_article(input: &str) -> &str {
    let trimmed = input.trim();
    let lower = trimmed.to_lowercase();
    for article in ["de ", "des ", "du ", "d'", "d’"] {
        if let Some(rest) = lower.strip_prefix(article) {
            return trimmed[trimmed.len() - rest.len()..].trim();
        }
    }
    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_iso_durations() {
        assert_eq!(parse_duration("PT25M"), Some(25));
        assert_eq!(parse_duration("PT1H30M"), Some(90));
        assert_eq!(parse_duration("PT2H"), Some(120));
        assert_eq!(parse_duration("P1D"), None);
        assert_eq!(parse_duration("PT0M"), None);
    }

    #[test]
    fn parses_metric_ingredients() {
        let item = parse_ingredient("600 g de courgettes");
        assert_eq!(item.name, "courgettes");
        assert_eq!(item.quantity, 600.0);
        assert_eq!(item.unit, Unit::G);
    }

    #[test]
    fn converts_centilitres_to_millilitres() {
        let item = parse_ingredient("40 cl de lait de coco");
        assert_eq!(item.name, "lait de coco");
        assert_eq!(item.quantity, 400.0);
        assert_eq!(item.unit, Unit::Ml);
    }

    #[test]
    fn converts_spoons_to_millilitres() {
        let item = parse_ingredient("2 cuillères à soupe d'huile d'olive");
        assert_eq!(item.name, "huile d'olive");
        assert_eq!(item.quantity, 30.0);
        assert_eq!(item.unit, Unit::Ml);
    }

    #[test]
    fn falls_back_to_pieces_without_unit() {
        let item = parse_ingredient("3 gousses d'ail");
        assert_eq!(item.name, "gousses d'ail");
        assert_eq!(item.quantity, 3.0);
        assert_eq!(item.unit, Unit::Piece);
    }

    #[test]
    fn does_not_read_g_inside_a_word() {
        // « gousses » ne doit pas être lu comme l'unité « g ».
        let item = parse_ingredient("2 gousses de vanille");
        assert_eq!(item.unit, Unit::Piece);
        assert_eq!(item.name, "gousses de vanille");
    }

    #[test]
    fn parses_fractions() {
        assert_eq!(parse_ingredient("1/2 citron").quantity, 0.5);
        assert_eq!(parse_ingredient("1 1/2 oignon").quantity, 1.5);
        assert_eq!(parse_ingredient("1,5 l d'eau").quantity, 1.5);
    }

    #[test]
    fn keeps_the_whole_line_without_a_quantity() {
        let item = parse_ingredient("sel et poivre");
        assert_eq!(item.name, "sel et poivre");
        assert_eq!(item.quantity, 1.0);
        assert_eq!(item.unit, Unit::Piece);
    }

    const PAGE: &str = r#"
<html><head>
<script type="application/ld+json">
{"@context":"https://schema.org","@graph":[
  {"@type":"WebPage","name":"bruit"},
  {"@type":["Recipe","Thing"],
   "name":"Ratatouille",
   "prepTime":"PT25M","cookTime":"PT45M",
   "image":{"url":"https://example.test/rata.jpg"},
   "recipeIngredient":["600 g de courgettes","2 gousses d'ail"],
   "recipeInstructions":[
     {"@type":"HowToStep","text":"Émincer l'ail."},
     {"@type":"HowToSection","itemListElement":[{"@type":"HowToStep","text":"Laisser mijoter."}]}
   ]}
]}
</script>
</head></html>
"#;

    #[test]
    fn finds_and_maps_a_recipe_from_a_graph() {
        let recipe = find_recipe(PAGE).expect("recette trouvée");
        let yaml = map_recipe(&recipe);

        assert_eq!(yaml.title, "Ratatouille");
        assert_eq!(yaml.prep_time_min, Some(25));
        assert_eq!(yaml.cook_time_min, Some(45));
        assert_eq!(yaml.photo.as_deref(), Some("https://example.test/rata.jpg"));
        assert_eq!(yaml.ingredients.len(), 2);
        assert_eq!(yaml.ingredients[0].name, "courgettes");
        // Les étapes des sections sont aplaties avec les étapes simples.
        assert_eq!(yaml.steps, ["Émincer l'ail.", "Laisser mijoter."]);
    }

    #[test]
    fn finds_a_recipe_with_an_html_escaped_script_type() {
        // Cas réel (Marmiton) : le type MIME est servi en entités HTML.
        let page = r#"<script type="application&#x2F;ld&#x2B;json">
            {"@type":"Recipe","name":"Ratatouille","recipeIngredient":["600 g de courgettes"]}
            </script>"#;
        let recipe = find_recipe(page).expect("recette trouvée malgré les entités");
        assert_eq!(map_recipe(&recipe).title, "Ratatouille");
    }

    #[test]
    fn ignores_pages_without_a_recipe() {
        assert!(
            find_recipe(r#"<script type="application/ld+json">{"@type":"Article"}</script>"#)
                .is_none()
        );
    }

    #[test]
    fn survives_an_invalid_json_ld_block() {
        let page = format!(r#"<script type="application/ld+json">{{ oops </script>{PAGE}"#);
        assert!(find_recipe(&page).is_some());
    }
}
