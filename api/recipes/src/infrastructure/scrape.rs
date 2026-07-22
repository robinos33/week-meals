//! Import d'une recette par URL (#61) : récupération de la page puis extraction
//! du **JSON-LD schema.org** (`<script type="application/ld+json">` avec
//! `@type: Recipe`) que publient la plupart des sites de cuisine, plutôt que
//! des sélecteurs HTML propres à chaque site — c'est le seul contrat réellement
//! partagé entre eux.
//!
//! Le résultat est un **brouillon à relire** : les quantités des sites sont du
//! texte libre (« 2 c. à soupe d'huile »), leur découpage en `amount`/`unit`
//! est heuristique.
//!
//! ## Garde SSRF
//! Exposé en API, c'est le serveur qui va chercher une URL fournie par le
//! client — on pourrait lui faire taper `http://localhost:…`, l'IP de
//! métadonnées du cloud, ou un service interne. [`HttpRecipeScraper::guarded`]
//! impose donc : https uniquement, résolution DNS vérifiée contre
//! loopback/plages privées/link-local **et épinglée** pour la connexion (contre
//! le DNS rebinding), redirections désactivées, taille de réponse bornée. La
//! variante [`HttpRecipeScraper::unrestricted`] lève ces gardes pour la CLI, où
//! c'est la machine de l'utilisateur qui fetch.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use kernel::Unit;
use regex::Regex;
use reqwest::redirect::Policy;
use reqwest::Url;
use serde_json::Value;

use crate::domain::{RecipeScraper, ScrapeError, ScrapedIngredient, ScrapedRecipe};

/// Taille maximale d'une page analysée : une recette ne pèse pas 5 Mo.
const MAX_BYTES: usize = 5 * 1024 * 1024;

/// Délai maximal d'une requête (déjà en place côté CLI historiquement).
const TIMEOUT: Duration = Duration::from_secs(20);

/// Certains sites refusent un User-Agent vide.
const USER_AGENT: &str =
    "Mozilla/5.0 (compatible; weekmeals/0.1; +https://github.com/robinos33/week-meals)";

/// Politique d'accès réseau.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Access {
    /// SSRF-safe : https, IP publiques, sans redirection. Pour l'API.
    Guarded,
    /// Sans garde : http/https, redirections suivies. Pour la CLI, où la machine
    /// de l'utilisateur fetch l'URL.
    Unrestricted,
}

/// Implémentation HTTP du [`RecipeScraper`].
pub struct HttpRecipeScraper {
    access: Access,
}

impl HttpRecipeScraper {
    /// Scraper gardé contre le SSRF, pour l'API (le serveur fetch l'URL client).
    #[must_use]
    pub fn guarded() -> Self {
        Self {
            access: Access::Guarded,
        }
    }

    /// Scraper sans garde, pour la CLI (la machine de l'utilisateur fetch).
    #[must_use]
    pub fn unrestricted() -> Self {
        Self {
            access: Access::Unrestricted,
        }
    }

    /// Récupère la page et renvoie son HTML (décodé, borné en taille).
    async fn fetch(&self, url: &str) -> Result<String, ScrapeError> {
        let parsed = Url::parse(url).map_err(|_| ScrapeError::InvalidUrl)?;
        let client = match self.access {
            Access::Guarded => guarded_client(&parsed).await?,
            Access::Unrestricted => {
                if !matches!(parsed.scheme(), "http" | "https") {
                    return Err(ScrapeError::InvalidUrl);
                }
                base_client()
                    .build()
                    .map_err(|_| ScrapeError::Unreachable)?
            }
        };

        let response = client
            .get(parsed)
            .send()
            .await
            .map_err(|_| ScrapeError::Unreachable)?;
        if !response.status().is_success() {
            return Err(ScrapeError::Unreachable);
        }
        // Rejet précoce si l'en-tête l'annonce ; `read_capped` défend si elle
        // ment ou manque.
        if response
            .content_length()
            .is_some_and(|len| len > MAX_BYTES as u64)
        {
            return Err(ScrapeError::TooLarge);
        }
        read_capped(response).await
    }
}

#[async_trait::async_trait]
impl RecipeScraper for HttpRecipeScraper {
    async fn scrape(&self, url: &str) -> Result<ScrapedRecipe, ScrapeError> {
        let html = self.fetch(url).await?;
        parse_recipe(&html).ok_or(ScrapeError::NoRecipe)
    }
}

/// Client de base : User-Agent, timeout. Suit les redirections par défaut.
fn base_client() -> reqwest::ClientBuilder {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(TIMEOUT)
}

/// Construit un client SSRF-safe : https imposé, IP résolue vérifiée puis
/// épinglée, redirections coupées.
async fn guarded_client(url: &Url) -> Result<reqwest::Client, ScrapeError> {
    if url.scheme() != "https" {
        return Err(ScrapeError::NotHttps);
    }
    let host = url.host_str().ok_or(ScrapeError::InvalidUrl)?;
    let port = url.port_or_known_default().unwrap_or(443);

    // On résout ici pour vérifier l'IP AVANT de se connecter ; l'adresse validée
    // est ensuite épinglée sur le client (`.resolve`) pour que la connexion cible
    // bien cette IP — pas une seconde résolution qui pourrait rebasculer vers une
    // IP interne (DNS rebinding). Redirections coupées : une 302 vers
    // `http://169.254.169.254` ne serait pas re-vérifiée.
    let addr = resolve_public(host, port).await?;

    base_client()
        .redirect(Policy::none())
        .resolve(host, addr)
        .build()
        .map_err(|_| ScrapeError::Unreachable)
}

/// Résout `host:port` et exige que **toutes** les adresses soient publiques
/// (une seule interne suffit à refuser), puis renvoie la première à épingler.
async fn resolve_public(host: &str, port: u16) -> Result<SocketAddr, ScrapeError> {
    let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host, port))
        .await
        .map_err(|_| ScrapeError::Unreachable)?
        .collect();
    let first = addrs.first().copied().ok_or(ScrapeError::Unreachable)?;
    if addrs.iter().all(|addr| is_public(addr.ip())) {
        Ok(first)
    } else {
        Err(ScrapeError::Blocked)
    }
}

/// Lit le corps en bornant la taille (défense si `Content-Length` ment ou
/// manque, notamment après décompression gzip).
async fn read_capped(mut response: reqwest::Response) -> Result<String, ScrapeError> {
    let mut buf: Vec<u8> = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| ScrapeError::Unreachable)?
    {
        if buf.len() + chunk.len() > MAX_BYTES {
            return Err(ScrapeError::TooLarge);
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

// --- Garde SSRF : classification des IP -----------------------------------

/// Une IP joignable publiquement (ni loopback, ni privée, ni link-local…).
fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => !is_blocked_v4(v4),
        IpAddr::V6(v6) => !is_blocked_v6(v6),
    }
}

/// Plages IPv4 interdites : privées, loopback, link-local, CGNAT, réservées…
fn is_blocked_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_unspecified()
        || ip.is_multicast()
        || a == 0 // 0.0.0.0/8 « this network »
        || a >= 240 // 240.0.0.0/4 réservé (Class E)
        || (a == 100 && (64..=127).contains(&b)) // 100.64.0.0/10 CGNAT
}

/// Plages IPv6 interdites : loopback, unique local, link-local, multicast, et
/// les IPv4 mappées (`::ffff:a.b.c.d`) évaluées comme leur v4.
fn is_blocked_v6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return is_blocked_v4(v4);
    }
    let first = ip.segments()[0];
    ip.is_loopback()
        || ip.is_unspecified()
        || ip.is_multicast()
        || (first & 0xfe00) == 0xfc00 // fc00::/7 unique local
        || (first & 0xffc0) == 0xfe80 // fe80::/10 link-local
}

// --- Extraction JSON-LD (pur) ---------------------------------------------

/// Extrait un brouillon de recette du JSON-LD d'une page. Pur et testable.
fn parse_recipe(html: &str) -> Option<ScrapedRecipe> {
    find_recipe(html).map(|recipe| map_recipe(&recipe))
}

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

/// Projette un nœud `Recipe` JSON-LD vers un brouillon.
fn map_recipe(recipe: &Value) -> ScrapedRecipe {
    ScrapedRecipe {
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

/// Découpe une ligne libre en montant / unité / nom.
///
/// Heuristique assumée : à défaut de quantité lisible, la ligne devient
/// « 1 pièce » du texte entier — le brouillon est de toute façon relu.
fn parse_ingredient(line: &str) -> ScrapedIngredient {
    let cleaned = line.replace('\u{a0}', " ");
    let cleaned = cleaned.trim();

    let Some((amount, rest)) = parse_amount(cleaned) else {
        return ScrapedIngredient {
            name: cleaned.to_owned(),
            amount: 1.0,
            unit: Unit::Piece,
        };
    };

    let (unit, factor, rest) = match_unit(rest);
    let name = strip_article(rest);
    // Un nom vide (« 3 œufs » où tout a été consommé) : on retombe sur la ligne
    // d'origine plutôt que de produire une entrée anonyme.
    let name = if name.is_empty() { cleaned } else { name };

    ScrapedIngredient {
        name: name.to_owned(),
        amount: (amount * factor).max(f64::MIN_POSITIVE),
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
        assert_eq!(item.amount, 600.0);
        assert_eq!(item.unit, Unit::G);
    }

    #[test]
    fn converts_centilitres_to_millilitres() {
        let item = parse_ingredient("40 cl de lait de coco");
        assert_eq!(item.name, "lait de coco");
        assert_eq!(item.amount, 400.0);
        assert_eq!(item.unit, Unit::Ml);
    }

    #[test]
    fn converts_spoons_to_millilitres() {
        let item = parse_ingredient("2 cuillères à soupe d'huile d'olive");
        assert_eq!(item.name, "huile d'olive");
        assert_eq!(item.amount, 30.0);
        assert_eq!(item.unit, Unit::Ml);
    }

    #[test]
    fn falls_back_to_pieces_without_unit() {
        let item = parse_ingredient("3 gousses d'ail");
        assert_eq!(item.name, "gousses d'ail");
        assert_eq!(item.amount, 3.0);
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
        assert_eq!(parse_ingredient("1/2 citron").amount, 0.5);
        assert_eq!(parse_ingredient("1 1/2 oignon").amount, 1.5);
        assert_eq!(parse_ingredient("1,5 l d'eau").amount, 1.5);
    }

    #[test]
    fn keeps_the_whole_line_without_a_quantity() {
        let item = parse_ingredient("sel et poivre");
        assert_eq!(item.name, "sel et poivre");
        assert_eq!(item.amount, 1.0);
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
        let recipe = parse_recipe(PAGE).expect("recette trouvée");

        assert_eq!(recipe.title, "Ratatouille");
        assert_eq!(recipe.prep_time_min, Some(25));
        assert_eq!(recipe.cook_time_min, Some(45));
        assert_eq!(
            recipe.photo.as_deref(),
            Some("https://example.test/rata.jpg")
        );
        assert_eq!(recipe.ingredients.len(), 2);
        assert_eq!(recipe.ingredients[0].name, "courgettes");
        // Les étapes des sections sont aplaties avec les étapes simples.
        assert_eq!(recipe.steps, ["Émincer l'ail.", "Laisser mijoter."]);
    }

    #[test]
    fn finds_a_recipe_with_an_html_escaped_script_type() {
        // Cas réel (Marmiton) : le type MIME est servi en entités HTML.
        let page = r#"<script type="application&#x2F;ld&#x2B;json">
            {"@type":"Recipe","name":"Ratatouille","recipeIngredient":["600 g de courgettes"]}
            </script>"#;
        let recipe = parse_recipe(page).expect("recette trouvée malgré les entités");
        assert_eq!(recipe.title, "Ratatouille");
    }

    #[test]
    fn ignores_pages_without_a_recipe() {
        assert!(
            parse_recipe(r#"<script type="application/ld+json">{"@type":"Article"}</script>"#)
                .is_none()
        );
    }

    #[test]
    fn survives_an_invalid_json_ld_block() {
        let page = format!(r#"<script type="application/ld+json">{{ oops </script>{PAGE}"#);
        assert!(parse_recipe(&page).is_some());
    }

    // --- Garde SSRF -------------------------------------------------------

    fn blocked(ip: &str) -> bool {
        !is_public(ip.parse().unwrap())
    }

    #[test]
    fn blocks_private_and_loopback_ipv4() {
        assert!(blocked("127.0.0.1")); // loopback
        assert!(blocked("10.0.0.1")); // privé
        assert!(blocked("172.16.5.4")); // privé
        assert!(blocked("192.168.1.1")); // privé
        assert!(blocked("169.254.169.254")); // link-local (métadonnées cloud)
        assert!(blocked("0.0.0.0")); // this network
        assert!(blocked("100.64.0.1")); // CGNAT
    }

    #[test]
    fn allows_public_ipv4() {
        assert!(!blocked("93.184.216.34")); // example.com
        assert!(!blocked("1.1.1.1"));
        assert!(!blocked("8.8.8.8"));
    }

    #[test]
    fn blocks_private_and_loopback_ipv6() {
        assert!(blocked("::1")); // loopback
        assert!(blocked("::")); // unspecified
        assert!(blocked("fe80::1")); // link-local
        assert!(blocked("fc00::1")); // unique local
        assert!(blocked("fd12:3456::1")); // unique local
        assert!(blocked("::ffff:127.0.0.1")); // IPv4 loopback mappée
        assert!(blocked("::ffff:10.0.0.1")); // IPv4 privée mappée
    }

    #[test]
    fn allows_public_ipv6() {
        assert!(!blocked("2606:4700:4700::1111")); // Cloudflare
        assert!(!blocked("2001:4860:4860::8888")); // Google
    }
}
