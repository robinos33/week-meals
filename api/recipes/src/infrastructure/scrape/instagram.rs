//! Import d'une recette depuis une publication **Instagram** (post ou reel).
//!
//! Instagram ne publie pas de JSON-LD `Recipe` : la recette est dans la
//! **légende**, en texte libre. On récupère donc la page publique
//! `…/embed/captioned/` de la publication — la seule qui rende la légende sans
//! session — puis on découpe ce texte à l'heuristique.
//!
//! ## Ce qu'on sait lire
//! Le format « Instagram cuisine » est assez stable pour être exploité :
//!
//! ```text
//! 🍝 Pâtes à la carbonara
//! Pour 4 personnes — Prépa 10 min, cuisson 15 min
//!
//! Ingrédients :
//! - 400 g de pâtes
//! - 200 g de guanciale
//!
//! Préparation :
//! 1. Faire bouillir l'eau.
//! 2. Snacker le guanciale.
//!
//! #recette #carbonara
//! ```
//!
//! Titre = première ligne, intertitres (« Ingrédients », « Préparation »)
//! délimitant les sections, puces et numérotations retirées, hashtags et
//! appels à l'action (« abonne-toi ») jetés. Sans intertitre, on retombe sur
//! la plus longue suite de lignes qui *ressemblent* à des ingrédients (puce ou
//! quantité en tête). Comme partout dans le scraping, le résultat est un
//! **brouillon à relire**.
//!
//! Un import sans le moindre ingrédient est refusé (`NoRecipe`) : une légende
//! de photo de vacances ne doit pas préremplir un formulaire de recette.
//!
//! ## Limites assumées
//! - La légende est le seul texte disponible : une recette qui vit uniquement
//!   dans la vidéo ou dans les commentaires ne sera pas importée.
//! - La photo pointe le CDN Instagram, dont les URLs sont signées et
//!   **expirent** : le brouillon la propose, à remplacer par un upload si on
//!   veut la garder.
//! - Instagram peut refuser de servir la page à un serveur (mur de connexion) ;
//!   l'import ressort alors en « page injoignable ».

use kernel::DEFAULT_SERVINGS;
use regex::Regex;
use reqwest::Url;
use serde::Deserialize;

use crate::domain::{normalize_title, ScrapedRecipe};

use super::parse_ingredient;

/// Fenêtre de recherche autour du marqueur JSON de la légende. Bornée pour ne
/// pas ré-échapper toute la page (les pages Instagram pèsent plusieurs centaines
/// de kilo-octets, la légende tient dans les premiers milliers de caractères).
const JSON_WINDOW_CHARS: usize = 20_000;

/// Longueur maximale d'un intertitre : au-delà, c'est une phrase qui parle de
/// préparation, pas un titre de section.
const HEADER_MAX_CHARS: usize = 45;

/// Longueur maximale d'un titre de recette repris de la première ligne.
const TITLE_MAX_CHARS: usize = 80;

/// Reconnaît une URL de publication Instagram et renvoie l'URL de sa page
/// « embed » — la seule variante publique qui rende la légende sans session.
///
/// Accepte les formes `/p/{code}`, `/reel/{code}`, `/reels/{code}`,
/// `/tv/{code}` et `/{compte}/p/{code}`, avec ou sans `www`, paramètres de
/// partage (`?igsh=…`) inclus. Renvoie `None` pour toute autre URL : l'appelant
/// retombe alors sur l'extraction JSON-LD habituelle.
///
/// L'URL renvoyée est **reconstruite** (jamais celle fournie) : le scraper de
/// l'API coupe les redirections, une forme canonique évite un 301 vers `www`.
pub fn embed_url(url: &Url) -> Option<String> {
    let host = url.host_str()?.to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host);
    if host != "instagram.com" && !host.ends_with(".instagram.com") {
        return None;
    }

    let segments: Vec<&str> = url
        .path_segments()?
        .filter(|segment| !segment.is_empty())
        .collect();
    let kind = segments
        .iter()
        .position(|segment| matches!(*segment, "p" | "reel" | "reels" | "tv"))?;
    let shortcode = segments.get(kind + 1)?;
    if !is_shortcode(shortcode) {
        return None;
    }
    Some(format!(
        "https://www.instagram.com/p/{shortcode}/embed/captioned/"
    ))
}

/// Identifiant court d'une publication : alphanumérique, `-` et `_`.
fn is_shortcode(candidate: &str) -> bool {
    (5..=32).contains(&candidate.chars().count())
        && candidate
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

/// Extrait un brouillon de recette d'une page `embed` Instagram. Pur et
/// testable : le réseau est fait par l'appelant.
pub fn parse_recipe(html: &str) -> Option<ScrapedRecipe> {
    let caption = caption(html)?;
    let mut recipe = parse_caption(&caption)?;
    recipe.photo = photo(html);
    Some(recipe)
}

// --- Extraction de la photo -----------------------------------------------

/// Hôtes du CDN Instagram : reconnaître une URL d'image sans dépendre d'une
/// classe CSS.
const CDN_HOSTS: &[&str] = &["cdninstagram.com", "fbcdn.net"];

/// Photo de la publication — la vignette que les messageries affichent en
/// aperçu du lien.
///
/// La page `embed` n'est pas la page du post : elle ne porte pas toujours les
/// balises OpenGraph. On essaie donc, dans l'ordre : `og:image` quand elle est
/// là, l'`<img>` du média intégré, le JSON de la page (`display_url`), puis
/// n'importe quelle image servie par le CDN Instagram.
///
/// Les URLs du CDN sont **signées et datées** : celle-ci finira par expirer.
/// C'est une photo de brouillon, à remplacer par un upload pour la garder.
fn photo(html: &str) -> Option<String> {
    og_content(html, "og:image")
        .or_else(|| media_image(html))
        .or_else(|| json_photo(html))
        .filter(|url| url.starts_with("https://"))
}

/// `src` de l'`<img>` du média intégré, sinon de la première image du CDN.
///
/// L'attribut est décodé : dans le HTML, les `&` des URLs signées arrivent en
/// `&amp;` — les recopier tels quels casserait la signature.
fn media_image(html: &str) -> Option<String> {
    let images = Regex::new(r"(?is)<img[^>]*>").ok()?;
    let mut cdn_fallback = None;
    for tag in images.find_iter(html) {
        let tag = tag.as_str();
        let Some(source) = attribute(tag, "src") else {
            continue;
        };
        if tag.contains("EmbeddedMediaImage") {
            return Some(source);
        }
        if cdn_fallback.is_none() && CDN_HOSTS.iter().any(|host| source.contains(host)) {
            cdn_fallback = Some(source);
        }
    }
    cdn_fallback
}

/// Valeur décodée d'un attribut HTML d'une balise.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let pattern = format!(r#"(?is)\b{name}\s*=\s*["']([^"']*)["']"#);
    let value = Regex::new(&pattern).ok()?.captures(tag)?.get(1)?.as_str();
    Some(decode_entities(value))
}

/// Photo depuis le JSON embarqué, sous les clés qu'Instagram y emploie.
fn json_photo(html: &str) -> Option<String> {
    ["display_url", "thumbnail_src", "display_src"]
        .into_iter()
        .find_map(|key| {
            // La clé est cherchée avec ses guillemets, sous les deux formes :
            // la fenêtre doit commencer sur le guillemet (ou son échappement)
            // pour que `json_string_after` la retrouve.
            let anchor = html
                .find(&format!(r#""{key}""#))
                .or_else(|| html.find(&format!(r#"\"{key}\""#)))?;
            let window = window_from(html, anchor, JSON_WINDOW_CHARS);
            let window = if window.contains(&format!(r#"\"{key}\""#)) {
                strip_one_escape_level(window)
            } else {
                window.to_owned()
            };
            json_string_after(&window, key)
        })
}

// --- Extraction de la légende ---------------------------------------------

/// Récupère la légende, en essayant les trois formes rencontrées sur la page
/// `embed` — le gabarit d'Instagram change sans préavis, on ne parie pas sur
/// une seule.
fn caption(html: &str) -> Option<String> {
    caption_from_json(html)
        .or_else(|| caption_from_div(html))
        .or_else(|| caption_from_og(html))
        .map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

/// Légende depuis le JSON embarqué dans la page (`edge_media_to_caption`).
///
/// Ce JSON est parfois lui-même *dans* une chaîne JSON (page servie via
/// `contextJSON`) : ses guillemets arrivent alors échappés. On retire dans ce
/// cas un niveau d'échappement avant de lire la valeur.
fn caption_from_json(html: &str) -> Option<String> {
    let anchor = html.find("edge_media_to_caption")?;
    let window = window_from(html, anchor, JSON_WINDOW_CHARS);
    let window = if window.contains(r#"\"text\""#) {
        strip_one_escape_level(window)
    } else {
        window.to_owned()
    };
    json_string_after(&window, "text")
}

/// Légende depuis le bloc HTML `<div class="Caption">` de la page `embed`.
fn caption_from_div(html: &str) -> Option<String> {
    // `\bCaption\b` ne matche ni `CaptionUsername` ni `CaptionComments`.
    let open = Regex::new(r#"(?is)<div[^>]*class\s*=\s*["'][^"']*\bCaption\b[^"']*["'][^>]*>"#)
        .ok()?
        .find(html)?;
    let rest = &html[open.end()..];
    // Le bloc des commentaires (« voir les N commentaires ») suit la légende :
    // il en marque la fin, les `<div>` imbriqués interdisant de compter les
    // balises fermantes.
    let end = Regex::new(r#"(?is)<div[^>]*class\s*=\s*["'][^"']*CaptionComments"#)
        .ok()?
        .find(rest)
        .map_or(rest.len(), |comments| comments.start());
    let text = html_to_text(&rest[..end]);
    (!text.trim().is_empty()).then_some(text)
}

/// Légende depuis `og:description`, dernier recours : Instagram y recopie la
/// légende **tronquée**, précédée du décompte de likes
/// (`123 likes, 4 comments - chef on May 1, 2026: "…"`).
fn caption_from_og(html: &str) -> Option<String> {
    let raw = og_content(html, "og:description")?;
    let text = raw
        .split_once(": \"")
        .map_or(raw.as_str(), |(_, quoted)| quoted)
        .trim()
        .trim_end_matches('"');
    (!text.is_empty()).then(|| text.to_owned())
}

/// Valeur de l'attribut `content` d'une balise `<meta>` OpenGraph.
fn og_content(html: &str, property: &str) -> Option<String> {
    let after = format!(
        r#"(?is)<meta[^>]*(?:property|name)\s*=\s*["']{property}["'][^>]*content\s*=\s*["']([^"']*)["']"#
    );
    let before = format!(
        r#"(?is)<meta[^>]*content\s*=\s*["']([^"']*)["'][^>]*(?:property|name)\s*=\s*["']{property}["']"#
    );
    [after, before]
        .iter()
        .find_map(|pattern| {
            let regex = Regex::new(pattern).ok()?;
            let value = regex.captures(html)?.get(1)?.as_str();
            Some(decode_entities(value))
        })
        .filter(|value| !value.trim().is_empty())
}

/// Transforme un fragment HTML de légende en texte : retours à la ligne des
/// `<br>`, balises retirées, entités décodées.
fn html_to_text(fragment: &str) -> String {
    // Le pseudo-lien du compte ouvre la légende : il donnerait un titre
    // « nom_du_compte ».
    let without_username = Regex::new(r"(?is)<a[^>]*CaptionUsername.*?</a>")
        .map(|regex| regex.replace_all(fragment, "").into_owned())
        .unwrap_or_else(|_| fragment.to_owned());
    let with_breaks = Regex::new(r"(?is)<br\s*/?>|</p>|</div>")
        .map(|regex| regex.replace_all(&without_username, "\n").into_owned())
        .unwrap_or(without_username);
    let without_tags = Regex::new(r"(?is)<[^>]+>")
        .map(|regex| regex.replace_all(&with_breaks, "").into_owned())
        .unwrap_or(with_breaks);
    decode_entities(&without_tags)
}

/// Décode les entités HTML rencontrées dans une légende (nommées courantes et
/// numériques). Une entité inconnue est laissée telle quelle.
fn decode_entities(text: &str) -> String {
    let named = [
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&apos;", "'"),
        ("&nbsp;", " "),
    ];
    let mut decoded = text.to_owned();
    for (entity, replacement) in named {
        decoded = decoded.replace(entity, replacement);
    }
    let Ok(numeric) = Regex::new(r"&#(x?)([0-9A-Fa-f]+);") else {
        return decoded;
    };
    numeric
        .replace_all(&decoded, |capture: &regex::Captures| {
            let radix = if capture[1].is_empty() { 10 } else { 16 };
            u32::from_str_radix(&capture[2], radix)
                .ok()
                .and_then(char::from_u32)
                .map_or_else(|| capture[0].to_owned(), String::from)
        })
        .into_owned()
}

/// Retire un niveau d'échappement (`\"` → `"`, `\\` → `\`) sans toucher aux
/// séquences JSON (`\n`, `\uXXXX`), qui restent à décoder ensuite.
fn strip_one_escape_level(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some(next @ ('"' | '\\' | '/')) => out.push(next),
            Some(next) => {
                out.push('\\');
                out.push(next);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Lit la chaîne JSON qui suit `"clé":` dans `input`.
fn json_string_after(input: &str, key: &str) -> Option<String> {
    let at = input.find(&format!("\"{key}\""))?;
    let colon = input[at..].find(':')? + at;
    let value = input[colon + 1..].trim_start();
    if !value.starts_with('"') {
        return None;
    }
    // `serde_json` s'arrête à la fin de la chaîne et gère les échappements,
    // paires de substitution des emojis comprises.
    let mut deserializer = serde_json::Deserializer::from_str(value);
    String::deserialize(&mut deserializer).ok()
}

/// Tranche `max_chars` caractères à partir de `start` (indice d'octet sur une
/// frontière de caractère), sans couper au milieu d'un caractère.
fn window_from(text: &str, start: usize, max_chars: usize) -> &str {
    let rest = &text[start..];
    let end = rest
        .char_indices()
        .take(max_chars)
        .last()
        .map_or(0, |(at, ch)| at + ch.len_utf8());
    &rest[..end]
}

// --- Découpage de la légende ----------------------------------------------

/// Section courante de la légende.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    /// Sous un intertitre « Ingrédients ».
    Ingredients,
    /// Sous un intertitre « Préparation ».
    Steps,
}

/// Découpe une légende en brouillon de recette. `None` si aucun ingrédient n'en
/// ressort : sans ingrédients, l'import n'a pas d'intérêt pour la liste de
/// courses, et la légende n'était probablement pas une recette.
///
/// Comme pour le JSON-LD, les quantités sont ramenées à [`DEFAULT_SERVINGS`]
/// quand la légende annonce « pour N personnes », pour que toutes les recettes
/// partagent la même base.
fn parse_caption(caption: &str) -> Option<ScrapedRecipe> {
    let lines: Vec<String> = caption
        .lines()
        .map(clean_line)
        .filter(|line| !line.is_empty())
        .collect();

    let mut title: Option<String> = None;
    let mut servings: Option<u32> = None;
    let mut prep_time_min: Option<u32> = None;
    let mut cook_time_min: Option<u32> = None;
    let mut ingredients: Vec<String> = Vec::new();
    let mut steps: Vec<String> = Vec::new();
    // Lignes hors de toute section : titre, blabla d'intro, et — sans
    // intertitre — la recette elle-même.
    let mut body: Vec<String> = Vec::new();
    let mut section: Option<Section> = None;

    for line in &lines {
        let normalized = normalize_title(line);
        let line_servings = parse_servings(&normalized);
        let (line_prep, line_cook) = parse_times(&normalized);
        servings = servings.or(line_servings);
        prep_time_min = prep_time_min.or(line_prep);
        cook_time_min = cook_time_min.or(line_cook);

        // Un intertitre ouvre une section — sauf s'il porte une durée
        // (« Préparation : 20 min » annonce un temps, pas les étapes).
        let timed = line_prep.is_some() || line_cook.is_some();
        if let Some(kind) = header_section(&normalized).filter(|_| !timed) {
            section = Some(kind);
            continue;
        }
        // Ligne de métadonnées (« ⏱ 15 min de prépa », « Pour 4 personnes ») :
        // l'information est déjà retenue, la ligne n'est pas du contenu.
        if (timed || line_servings.is_some()) && is_metadata_only(&normalized) {
            continue;
        }
        if is_call_to_action(&normalized) {
            continue;
        }
        // Ligne purement décorative : emojis, ou le « . » que beaucoup de
        // comptes intercalent pour aérer la légende.
        if !strip_decorations(line).chars().any(char::is_alphanumeric) {
            continue;
        }

        match section {
            Some(Section::Ingredients) => ingredients.push(line.clone()),
            Some(Section::Steps) => steps.push(line.clone()),
            None if title.is_none() => title = Some(title_from(&strip_decorations(line))),
            None => body.push(line.clone()),
        }
    }

    // Sans intertitre « Ingrédients », on cherche la plus longue suite de
    // lignes qui en ont la forme ; ce qui suit devient les étapes.
    if ingredients.is_empty() {
        if let Some(run) = ingredient_run(&body) {
            ingredients = body[run.clone()].to_vec();
            steps = body[run.end..].iter().cloned().chain(steps).collect();
        }
    }

    let ingredients: Vec<String> = ingredients
        .iter()
        .map(|line| expand_fractions(&strip_decorations(strip_bullet(line).1)))
        .filter(|line| !line.is_empty())
        .collect();
    if ingredients.is_empty() {
        return None;
    }

    let scale = servings.map_or(1.0, |from| f64::from(DEFAULT_SERVINGS) / f64::from(from));
    Some(ScrapedRecipe {
        title: title.unwrap_or_else(|| "Sans titre".to_owned()),
        prep_time_min,
        cook_time_min,
        photo: None,
        ingredients: ingredients
            .iter()
            .map(|line| {
                let mut ingredient = parse_ingredient(line);
                ingredient.amount = (ingredient.amount * scale).max(f64::MIN_POSITIVE);
                ingredient
            })
            .collect(),
        steps: steps
            .iter()
            .map(|line| strip_decorations(strip_step_number(strip_bullet(line).1)))
            .filter(|line| !line.is_empty())
            .collect(),
        servings: DEFAULT_SERVINGS,
    })
}

/// Nettoie une ligne de légende : espaces insécables, hashtags de fin, trim.
fn clean_line(line: &str) -> String {
    let line = line.replace('\u{a0}', " ");
    let without_tags = Regex::new(r"(\s*#[^\s#]+)+\s*$")
        .map(|regex| regex.replace(&line, "").into_owned())
        .unwrap_or(line);
    without_tags.trim().to_owned()
}

/// Titre repris de la première ligne, borné à [`TITLE_MAX_CHARS`] : on coupe à
/// la première ponctuation forte, sinon au dernier mot entier.
fn title_from(line: &str) -> String {
    if line.chars().count() <= TITLE_MAX_CHARS {
        return line.to_owned();
    }
    let head = window_from(line, 0, TITLE_MAX_CHARS);
    let cut = head.find(['.', '!', '?', ':', ';']).map_or_else(
        || head.rsplit_once(' ').map_or(head, |(start, _)| start),
        |at| &head[..at],
    );
    cut.trim().to_owned()
}

/// Intertitre de section, si la ligne en est un.
fn header_section(normalized: &str) -> Option<Section> {
    if normalized.chars().count() > HEADER_MAX_CHARS {
        return None;
    }
    const INGREDIENTS: &[&str] = &[
        "ingredient",
        "il vous faut",
        "il te faut",
        "il faut",
        "liste de courses",
        "la liste",
        "shopping list",
    ];
    const STEPS: &[&str] = &[
        "preparation",
        "etapes",
        "instructions",
        "realisation",
        "methode",
        "marche a suivre",
        "on y va",
        "en cuisine",
        "la recette",
    ];
    if INGREDIENTS.iter().any(|key| normalized.contains(key)) {
        return Some(Section::Ingredients);
    }
    STEPS
        .iter()
        .any(|key| normalized.contains(key))
        .then_some(Section::Steps)
}

/// Une ligne qui ne porte qu'un temps ou un nombre de parts : une fois les
/// nombres, unités et mots-clés retirés, il ne reste presque rien.
fn is_metadata_only(normalized: &str) -> bool {
    const KEYWORDS: &[&str] = &[
        "temps",
        "total",
        "repos",
        "personnes",
        "personne",
        "parts",
        "part",
        "portions",
        "portion",
        "pers",
        "pour",
        "min",
        "mn",
        "minutes",
        "minute",
        "heures",
        "heure",
        "environ",
        "chrono",
        "cuisson",
        "cuire",
        "four",
        "recette",
        "facile",
        "rapide",
    ];
    normalized
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| word.chars().count() >= 3)
        .filter(|word| !word.chars().all(|ch| ch.is_ascii_digit()))
        .filter(|word| !word.starts_with("prep") && !word.starts_with("cuiss"))
        .filter(|word| !KEYWORDS.contains(word))
        .count()
        <= 1
}

/// Appel à l'action de fin de légende — pas du contenu de recette.
fn is_call_to_action(normalized: &str) -> bool {
    const PATTERNS: &[&str] = &[
        "abonne",
        "abonnez",
        "suis moi",
        "suivez moi",
        "lien en bio",
        "enregistre cette recette",
        "enregistre la recette",
        "partage cette recette",
        "double tap",
        "like si",
        "en commentaire",
        "dis moi en",
    ];
    PATTERNS.iter().any(|pattern| normalized.contains(pattern))
}

/// Nombre de parts annoncé (« pour 4 personnes », « 6 parts »).
fn parse_servings(normalized: &str) -> Option<u32> {
    let regex = Regex::new(r"(\d+)\s*(?:personnes?|parts?|portions?|pers\b|convives?)").ok()?;
    regex
        .captures(normalized)?
        .get(1)?
        .as_str()
        .parse()
        .ok()
        .filter(|count| *count > 0)
}

/// Temps de préparation et de cuisson lus autour de leurs mots-clés.
fn parse_times(normalized: &str) -> (Option<u32>, Option<u32>) {
    let prep = duration_near(normalized, "prep");
    let cook = duration_near(normalized, "cuisson").or_else(|| duration_near(normalized, "cuire"));
    (prep, cook)
}

/// Fenêtre de recherche d'une durée autour d'un mot-clé, en caractères.
const DURATION_WINDOW_CHARS: usize = 40;

/// Cherche une durée juste après le mot-clé, sinon juste avant (« 15 min de
/// préparation »).
fn duration_near(text: &str, keyword: &str) -> Option<u32> {
    let at = text.find(keyword)?;
    let after = window_from(text, at + keyword.len(), DURATION_WINDOW_CHARS);
    if let Some(minutes) = first_duration(after) {
        return Some(minutes);
    }
    let before = &text[..at];
    let start = before
        .char_indices()
        .rev()
        .take(DURATION_WINDOW_CHARS)
        .last()
        .map_or(0, |(index, _)| index);
    first_duration(&before[start..])
}

/// Première durée d'un texte, en minutes (`45 min`, `1h`, `1h30`, `2 heures`).
fn first_duration(text: &str) -> Option<u32> {
    let hours = Regex::new(r"(\d+)\s*h(?:eures?)?\s*(\d{1,2})?\b").ok()?;
    if let Some(capture) = hours.captures(text) {
        let h: u32 = capture.get(1)?.as_str().parse().ok()?;
        let m: u32 = capture
            .get(2)
            .and_then(|minutes| minutes.as_str().parse().ok())
            .unwrap_or(0);
        // Saturant : une légende peut annoncer n'importe quel nombre d'heures,
        // et un débordement paniquerait en debug. Une valeur absurde ressortira
        // en `TimeOutOfRange` (422) à l'enregistrement, pas en 500 à l'import.
        let total = h.saturating_mul(60).saturating_add(m);
        return (total > 0).then_some(total);
    }
    let minutes = Regex::new(r"(\d+)\s*(?:min|mn|minutes?)\b").ok()?;
    minutes
        .captures(text)?
        .get(1)?
        .as_str()
        .parse()
        .ok()
        .filter(|count| *count > 0)
}

/// Caractères purement décoratifs : emojis, symboles, puces, sélecteurs de
/// variante. Retirés du contenu retenu.
fn is_decoration(ch: char) -> bool {
    matches!(
        u32::from(ch),
        0x200D                  // liant sans chasse des emojis composés
            | 0x2022..=0x2023   // puces
            | 0x2190..=0x2BFF   // flèches, symboles, dingbats
            | 0xFE00..=0xFE0F   // sélecteurs de variante
            | 0x1F000..=0x1FAFF // emojis
    )
}

/// Caractères de bordure sans intérêt en début ou fin de ligne.
const EDGE: &[char] = &[
    '-', '–', '—', '*', '·', ':', ';', ',', '_', '=', '"', '\'', ' ',
];

/// Retire décorations et bordures, et normalise les espaces.
fn strip_decorations(line: &str) -> String {
    let stripped: String = line.chars().filter(|ch| !is_decoration(*ch)).collect();
    stripped
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(EDGE)
        .to_owned()
}

/// Détache une puce de tête. Renvoie `(la ligne en avait une, le reste)`.
fn strip_bullet(line: &str) -> (bool, &str) {
    let trimmed = line.trim_start();
    let Some(first) = trimmed.chars().next() else {
        return (false, trimmed);
    };
    if !matches!(
        first,
        '-' | '–' | '—' | '*' | '•' | '·' | '▪' | '▫' | '→' | '‣' | '>'
    ) {
        return (false, trimmed);
    }
    let rest = trimmed[first.len_utf8()..].trim_start();
    // « -5 min » n'est pas une puce ; une puce est suivie d'un espace ou colle
    // à du texte, pas à un chiffre.
    if trimmed[first.len_utf8()..].starts_with(|ch: char| ch.is_ascii_digit()) {
        return (false, trimmed);
    }
    (true, rest)
}

/// Retire la numérotation d'une étape (« 1. », « 2) », « 3 - »).
fn strip_step_number(line: &str) -> &str {
    let Ok(regex) = Regex::new(r"^\s*\d{1,2}\s*[.)°/-]\s+") else {
        return line;
    };
    regex
        .find(line)
        .map_or(line, |numbering| &line[numbering.end()..])
}

/// Écrit les fractions typographiques sous une forme que le découpage
/// d'ingrédient sait lire (« ½ citron » → « 1/2 citron »).
fn expand_fractions(line: &str) -> String {
    const FRACTIONS: &[(char, &str)] = &[
        ('½', "1/2"),
        ('⅓', "1/3"),
        ('⅔', "2/3"),
        ('¼', "1/4"),
        ('¾', "3/4"),
        ('⅕', "1/5"),
        ('⅛', "1/8"),
    ];
    let mut expanded = line.to_owned();
    for (glyph, text) in FRACTIONS {
        if expanded.contains(*glyph) {
            expanded = expanded.replace(*glyph, text);
        }
    }
    expanded
}

/// Une ligne qui a la forme d'un ingrédient : puce ou quantité en tête, et
/// courte.
fn looks_like_ingredient(line: &str) -> bool {
    let (bullet, rest) = strip_bullet(line);
    let rest = strip_decorations(rest);
    let starts_with_amount = rest
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_digit() || "½⅓⅔¼¾⅕⅛".contains(ch));
    (bullet || starts_with_amount) && (1..=70).contains(&rest.chars().count())
}

/// Plus longue suite de lignes consécutives qui ressemblent à des ingrédients
/// (au moins deux : une ligne isolée serait trop souvent une phrase).
fn ingredient_run(lines: &[String]) -> Option<std::ops::Range<usize>> {
    let mut best: Option<std::ops::Range<usize>> = None;
    let mut start: Option<usize> = None;
    for (index, line) in lines.iter().enumerate() {
        if looks_like_ingredient(line) {
            start.get_or_insert(index);
            continue;
        }
        if let Some(from) = start.take() {
            best = longest(best, from..index);
        }
    }
    if let Some(from) = start {
        best = longest(best, from..lines.len());
    }
    best.filter(|run| run.len() >= 2)
}

/// Garde la plus longue de deux suites.
fn longest(
    current: Option<std::ops::Range<usize>>,
    candidate: std::ops::Range<usize>,
) -> Option<std::ops::Range<usize>> {
    match current {
        Some(best) if best.len() >= candidate.len() => Some(best),
        _ => Some(candidate),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kernel::Unit;

    fn url(raw: &str) -> Url {
        Url::parse(raw).unwrap()
    }

    #[test]
    fn recognizes_post_and_reel_urls() {
        let expected = "https://www.instagram.com/p/CxYz-12_ab/embed/captioned/";
        for raw in [
            "https://www.instagram.com/p/CxYz-12_ab/",
            "https://instagram.com/reel/CxYz-12_ab",
            "https://www.instagram.com/reels/CxYz-12_ab/?igsh=abc123",
            "https://www.instagram.com/tv/CxYz-12_ab/",
            "https://www.instagram.com/chef.cuisine/p/CxYz-12_ab/",
        ] {
            assert_eq!(embed_url(&url(raw)).as_deref(), Some(expected), "{raw}");
        }
    }

    #[test]
    fn ignores_non_post_urls() {
        for raw in [
            "https://www.instagram.com/chef.cuisine/",
            "https://www.instagram.com/p/",
            "https://marmiton.org/p/CxYz-12_ab/",
            "https://not-instagram.com/p/CxYz-12_ab/",
        ] {
            assert!(embed_url(&url(raw)).is_none(), "{raw}");
        }
    }

    const CAPTION: &str = "🍝 Pâtes à la carbonara\n\
        Pour 4 personnes ⏱️ Prépa 10 min, cuisson 15 min\n\
        \n\
        Ingrédients :\n\
        - 400 g de pâtes\n\
        - 200 g de guanciale\n\
        - 4 jaunes d'œufs\n\
        \n\
        Préparation :\n\
        1. Faire bouillir l'eau.\n\
        2. Snacker le guanciale.\n\
        \n\
        Abonne-toi pour plus de recettes ! 💪\n\
        #recette #carbonara";

    #[test]
    fn parses_a_captioned_recipe() {
        let recipe = parse_caption(CAPTION).expect("recette trouvée");

        assert_eq!(recipe.title, "Pâtes à la carbonara");
        assert_eq!(recipe.prep_time_min, Some(10));
        assert_eq!(recipe.cook_time_min, Some(15));
        assert_eq!(
            recipe.steps,
            ["Faire bouillir l'eau.", "Snacker le guanciale."]
        );
        // « Pour 4 personnes » : quantités ramenées à la base 2.
        assert_eq!(recipe.servings, 2);
        assert_eq!(recipe.ingredients.len(), 3);
        assert_eq!(recipe.ingredients[0].name, "pâtes");
        assert_eq!(recipe.ingredients[0].amount, 200.0);
        assert_eq!(recipe.ingredients[0].unit, Unit::G);
        assert_eq!(recipe.ingredients[2].name, "jaunes d'œufs");
        assert_eq!(recipe.ingredients[2].amount, 2.0);
    }

    #[test]
    fn falls_back_to_the_longest_ingredient_run_without_headers() {
        let caption = "Salade de lentilles ✨\n\
            Une tuerie pour l'été.\n\
            250 g de lentilles\n\
            1 oignon rouge\n\
            2 cuillères à soupe d'huile d'olive\n\
            Cuire les lentilles 20 minutes puis rincer à l'eau froide.\n\
            Mélanger le tout et servir frais.";
        let recipe = parse_caption(caption).expect("recette trouvée");

        assert_eq!(recipe.title, "Salade de lentilles");
        assert_eq!(recipe.ingredients.len(), 3);
        assert_eq!(recipe.ingredients[0].name, "lentilles");
        assert_eq!(recipe.ingredients[2].amount, 30.0);
        assert_eq!(recipe.ingredients[2].unit, Unit::Ml);
        assert_eq!(
            recipe.steps,
            [
                "Cuire les lentilles 20 minutes puis rincer à l'eau froide.",
                "Mélanger le tout et servir frais."
            ]
        );
    }

    #[test]
    fn reads_typographic_fractions() {
        let caption = "Vinaigrette\nIngrédients :\n- ½ citron\n- 3 cuillères à soupe d'huile";
        let recipe = parse_caption(caption).expect("recette trouvée");
        assert_eq!(recipe.ingredients[0].amount, 0.5);
        assert_eq!(recipe.ingredients[0].name, "citron");
    }

    #[test]
    fn keeps_a_cooking_step_that_mentions_a_duration() {
        // « Faire cuire 30 min à feu doux » renseigne le temps de cuisson sans
        // disparaître des étapes : ce n'est pas une ligne de métadonnées.
        let caption = "Riz au lait\nIngrédients :\n- 200 g de riz\n- 1 l de lait\n\
            Préparation :\nFaire cuire 30 min à feu doux en remuant.";
        let recipe = parse_caption(caption).expect("recette trouvée");
        assert_eq!(recipe.cook_time_min, Some(30));
        assert_eq!(recipe.steps, ["Faire cuire 30 min à feu doux en remuant."]);
    }

    #[test]
    fn drops_a_metadata_only_line() {
        let caption = "Poulet rôti\n⏱️ Préparation : 15 min\nCuisson : 1h30\n\
            Ingrédients :\n- 1 poulet\n- 3 pommes de terre";
        let recipe = parse_caption(caption).expect("recette trouvée");
        assert_eq!(recipe.prep_time_min, Some(15));
        assert_eq!(recipe.cook_time_min, Some(90));
        assert!(recipe.steps.is_empty());
    }

    #[test]
    fn shortens_a_long_first_line_into_a_title() {
        let caption = "Cette recette de gratin de courgettes est celle que ma grand-mère \
            faisait chaque été. Un régal !\nIngrédients :\n- 4 courgettes\n- 20 cl de crème";
        let recipe = parse_caption(caption).expect("recette trouvée");
        assert_eq!(
            recipe.title,
            "Cette recette de gratin de courgettes est celle que ma grand-mère faisait"
        );
    }

    #[test]
    fn drops_decorative_separator_lines() {
        // Le « . » isolé et les lignes d'emojis servent à aérer la légende.
        let caption = "Tiramisu\nIngrédients :\n- 250 g de mascarpone\n- 3 œufs\n\
            Préparation :\nMonter les blancs en neige.\n.\n🍰🍰🍰\n.";
        let recipe = parse_caption(caption).expect("recette trouvée");
        assert_eq!(recipe.steps, ["Monter les blancs en neige."]);
    }

    #[test]
    fn survives_an_absurd_duration() {
        // Une légende peut annoncer n'importe quoi : pas de débordement.
        let caption = "Cassoulet\nCuisson : 999999999 h 59\nIngrédients :\n- 1 kg de haricots";
        let recipe = parse_caption(caption).expect("recette trouvée");
        assert_eq!(recipe.cook_time_min, Some(u32::MAX));
    }

    #[test]
    fn rejects_a_caption_without_ingredients() {
        assert!(parse_caption("Belle journée au marché ☀️ #dimanche").is_none());
        assert!(parse_caption("").is_none());
    }

    // --- Extraction depuis la page embed ----------------------------------

    #[test]
    fn reads_the_caption_from_the_embed_markup() {
        let html = r#"<html><head>
            <meta property="og:image" content="https://cdn.test/photo.jpg"/>
            </head><body>
            <div class="Caption">
              <a class="CaptionUsername" href="/chef/"><div class="UsernameText">chef</div></a>
              Tarte aux pommes<br>Ingr&#233;dients :<br>- 4 pommes<br>- 100 g de sucre
              <div class="CaptionComments">voir les 12 commentaires</div>
            </div></body></html>"#;
        let recipe = parse_recipe(html).expect("recette trouvée");

        assert_eq!(recipe.title, "Tarte aux pommes");
        assert_eq!(recipe.photo.as_deref(), Some("https://cdn.test/photo.jpg"));
        assert_eq!(recipe.ingredients.len(), 2);
        assert_eq!(recipe.ingredients[1].name, "sucre");
    }

    #[test]
    fn reads_the_caption_from_the_embedded_json() {
        let html = r#"<script>window.__additionalDataLoaded('extra',
            {"edge_media_to_caption":{"edges":[{"node":{"text":
            "Soupe de potiron 🎃\nIngrédients :\n- 1 potiron\n- 50 cl de bouillon"
            }}]}});</script>"#;
        let recipe = parse_recipe(html).expect("recette trouvée");

        assert_eq!(recipe.title, "Soupe de potiron");
        assert_eq!(recipe.ingredients[1].name, "bouillon");
        assert_eq!(recipe.ingredients[1].amount, 500.0);
    }

    #[test]
    fn reads_the_caption_from_doubly_escaped_json() {
        // Page servie via `contextJSON` : le JSON est dans une chaîne JSON.
        let html = concat!(
            r#"<script>requireLazy(["ScheduledServerJS"],function(){handle("#,
            r#"{"contextJSON":"{\"edge_media_to_caption\":{\"edges\":[{\"node\":{\"text\":"#,
            r#"\"Gratin dauphinois\\nIngr\\u00e9dients :\\n- 1 kg de pommes de terre"#,
            r#"\\n- 25 cl de cr\\u00e8me\"}}]}}"});});</script>"#,
        );
        let recipe = parse_recipe(html).expect("recette trouvée");

        assert_eq!(recipe.title, "Gratin dauphinois");
        assert_eq!(recipe.ingredients[0].name, "pommes de terre");
        assert_eq!(recipe.ingredients[0].unit, Unit::Kg);
    }

    #[test]
    fn falls_back_to_the_open_graph_description() {
        let html = r#"<meta property="og:description"
            content="123 likes, 4 comments - chef on May 1, 2026: &quot;Houmous maison
- 1 boite de pois chiches
- 2 cuillères à soupe de tahini&quot;">"#;
        let recipe = parse_recipe(html).expect("recette trouvée");

        assert_eq!(recipe.title, "Houmous maison");
        assert_eq!(recipe.ingredients.len(), 2);
    }

    /// Page `embed` telle qu'Instagram la sert : pas de balise OpenGraph, le
    /// média dans un `<img class="EmbeddedMediaImage">` dont l'URL signée a ses
    /// `&` échappés.
    const EMBED_PAGE: &str = r#"<html><body>
        <div class="EmbedFrame">
          <img class="EmbeddedMediaImage" referrerpolicy="origin-when-cross-origin"
               src="https://scontent.cdninstagram.com/v/t51.29350-15/photo.jpg?stp=dst-jpg&amp;_nc_ht=scontent.cdninstagram.com&amp;oh=00_AY&amp;oe=65F0"/>
          <div class="Caption">Chili sin carne<br>Ingrédients :<br>- 400 g de haricots rouges<br>- 1 poivron</div>
        </div></body></html>"#;

    #[test]
    fn reads_the_photo_from_the_embedded_media_image() {
        let recipe = parse_recipe(EMBED_PAGE).expect("recette trouvée");
        // Les `&amp;` sont décodés : une URL signée recopiée telle quelle
        // serait rejetée par le CDN.
        assert_eq!(
            recipe.photo.as_deref(),
            Some(
                "https://scontent.cdninstagram.com/v/t51.29350-15/photo.jpg\
                 ?stp=dst-jpg&_nc_ht=scontent.cdninstagram.com&oh=00_AY&oe=65F0"
            )
        );
    }

    #[test]
    fn prefers_the_open_graph_image_when_the_page_has_one() {
        let html = format!(
            r#"<meta property="og:image" content="https://scontent.cdninstagram.com/apercu.jpg">{EMBED_PAGE}"#
        );
        let recipe = parse_recipe(&html).expect("recette trouvée");
        assert_eq!(
            recipe.photo.as_deref(),
            Some("https://scontent.cdninstagram.com/apercu.jpg")
        );
    }

    #[test]
    fn reads_the_photo_from_the_embedded_json() {
        let html = concat!(
            r#"<script>handle({"contextJSON":"{\"display_url\":"#,
            r#"\"https://scontent.cdninstagram.com/plat.jpg?oe=1\\u0026oh=2\","#,
            r#"\"edge_media_to_caption\":{\"edges\":[{\"node\":{\"text\":"#,
            r#"\"Curry\\nIngr\\u00e9dients :\\n- 300 g de pois chiches\\n- 20 cl de lait de coco\"}}]}}"});</script>"#,
        );
        let recipe = parse_recipe(html).expect("recette trouvée");
        assert_eq!(
            recipe.photo.as_deref(),
            Some("https://scontent.cdninstagram.com/plat.jpg?oe=1&oh=2")
        );
    }

    #[test]
    fn ignores_a_decorative_image_that_is_not_the_media() {
        // Un logo servi hors CDN ne doit pas devenir la photo de la recette.
        let html = r#"<img class="Logo" src="https://static.test/logo.svg"/>
            <div class="Caption">Soupe<br>Ingrédients :<br>- 3 carottes<br>- 1 oignon</div>"#;
        let recipe = parse_recipe(html).expect("recette trouvée");
        assert_eq!(recipe.photo, None);
    }

    #[test]
    fn ignores_a_page_without_a_caption() {
        assert!(parse_recipe("<html><body>Connectez-vous</body></html>").is_none());
    }
}

#[cfg(test)]
mod fuzz {
    /// Garde-fou contre les paniques de découpage : le module tranche beaucoup
    /// de chaînes (fenêtres, indices de regex), et une légende Instagram est du
    /// texte que personne ne contrôle. Un échantillon suffit à réveiller un
    /// `&text[i..]` posé sur une frontière de caractère invalide ; la campagne
    /// initiale (20 000 tirages) est passée sans panique.
    ///
    /// Générateur pseudo-aléatoire déterministe (xorshift) — pas de dépendance.
    fn next(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    #[test]
    fn never_panics_on_arbitrary_input() {
        const ALPHABET: &[&str] = &[
            "é",
            "🍝",
            "\n",
            "<div class=\"Caption\">",
            "</div>",
            "<br>",
            "&#x2F;",
            "\\\"text\\\":",
            "edge_media_to_caption",
            "\"text\":\"",
            "og:description",
            "1/2",
            "½",
            "- ",
            "1) ",
            "999999999 h",
            "Ingrédients :",
            "Préparation",
            "pour 4 personnes",
            "min",
            "h",
            ":",
            "\"",
            "\\",
            "#tag",
            "•",
            "\u{200d}",
            "\u{fe0f}",
            "0",
            "9",
            " ",
            "meta property=",
        ];
        let mut state = 0x2026_0803_dead_beef;
        for _ in 0..200 {
            let length = (next(&mut state) % 40) as usize;
            let mut input = String::new();
            for _ in 0..length {
                input.push_str(ALPHABET[(next(&mut state) as usize) % ALPHABET.len()]);
            }
            let _ = super::parse_recipe(&input);
            let _ = super::parse_caption(&input);
        }
    }
}
