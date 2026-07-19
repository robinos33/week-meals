//! `weekmeals` — outils en ligne de commande de Week Meals.
//!
//! Aujourd'hui : import / export / seed des recettes au format YAML (contrat de
//! seed, cf. `data/recipes/*.yaml` et ADR-0003). L'import est **idempotent** :
//! il fait un upsert par titre au sein du foyer, pour rejouer un seed sans
//! créer de doublons.

mod ingredient_yaml;
mod recipe_yaml;
mod scrape;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use kernel::{HouseholdId, RecipeId, DEMO_HOUSEHOLD_ID};
use recipes::domain::RecipeRepository;
use recipes::infrastructure::SqlxRecipeRepository;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use ingredient_yaml::ReferenceFile;
use recipe_yaml::RecipeYaml;
use shopping_list::domain::IngredientReference;
use shopping_list::infrastructure::SqlxReferenceRepository;

/// Outils Week Meals.
#[derive(Parser)]
#[command(
    name = "weekmeals",
    version,
    about = "Import / export / seed des recettes (YAML)."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Importe une ou plusieurs recettes YAML (upsert idempotent par titre).
    Import {
        /// Fichiers `.yaml` à importer.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Foyer cible (UUID). Défaut : le foyer de démonstration.
        #[arg(long)]
        household: Option<Uuid>,
    },
    /// Exporte les recettes d'un foyer en YAML (stdout ou dossier).
    Export {
        /// Dossier de sortie (un fichier par recette). Défaut : stdout.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Foyer source (UUID). Défaut : le foyer de démonstration.
        #[arg(long)]
        household: Option<Uuid>,
    },
    /// Seede les recettes d'un dossier (`data/recipes` par défaut) dans le foyer démo.
    Seed {
        /// Dossier des seeds YAML.
        #[arg(long, default_value = "data/recipes")]
        dir: PathBuf,
        /// Foyer cible (UUID). Défaut : le foyer de démonstration.
        #[arg(long)]
        household: Option<Uuid>,
    },
    /// Seede le référentiel d'ingrédients (poids moyens) — global, pas par foyer.
    SeedIngredients {
        /// Fichier YAML du référentiel.
        #[arg(long, default_value = "data/ingredients.yaml")]
        file: PathBuf,
    },
    /// Extrait une recette d'une page web vers le YAML de seed (brouillon à relire).
    Scrape {
        /// URL de la page de recette.
        url: String,
        /// Fichier de sortie. Défaut : stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Charge `.env` (dev local) ; en CI/prod l'environnement est déjà injecté.
    let _ = dotenvy::dotenv();
    let cli = Cli::parse();

    // Le scraping ne touche pas la base : inutile d'exiger DATABASE_URL.
    if let Command::Scrape { url, out } = &cli.command {
        let recipe = scrape::scrape(url).await?;
        let yaml = serde_yaml::to_string(&recipe).context("sérialisation YAML")?;
        match out {
            Some(path) => {
                std::fs::write(path, &yaml)
                    .with_context(|| format!("écriture de {}", path.display()))?;
                println!(
                    "Recette écrite dans {} — à relire avant import.",
                    path.display()
                );
            }
            None => print!("{yaml}"),
        }
        return Ok(());
    }

    let database_url = std::env::var("DATABASE_URL").context("DATABASE_URL doit être défini")?;
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .context("connexion à la base de données")?;
    let repo = SqlxRecipeRepository::new(pool.clone());

    match cli.command {
        Command::Import { paths, household } => {
            let household = resolve_household(household);
            let count = import_files(&repo, household, &paths).await?;
            println!("{count} recette(s) importée(s) dans le foyer {household}.");
        }
        Command::Seed { dir, household } => {
            let household = resolve_household(household);
            let paths = yaml_files(&dir)?;
            if paths.is_empty() {
                bail!("aucun fichier .yaml dans {}", dir.display());
            }
            let count = import_files(&repo, household, &paths).await?;
            println!("{count} recette(s) seedée(s) dans le foyer {household}.");
        }
        // Traité plus haut, avant l'ouverture du pool.
        Command::Scrape { .. } => unreachable!("scrape est traité avant la base"),
        Command::Export { out, household } => {
            let household = resolve_household(household);
            export(&repo, household, out.as_deref()).await?;
        }
        Command::SeedIngredients { file } => {
            let raw = std::fs::read_to_string(&file)
                .with_context(|| format!("lecture de {}", file.display()))?;
            let doc: ReferenceFile = serde_yaml::from_str(&raw)
                .with_context(|| format!("YAML invalide : {}", file.display()))?;
            let references: Vec<IngredientReference> =
                doc.ingredients.into_iter().map(Into::into).collect();
            let count = SqlxReferenceRepository::new(pool)
                .upsert_all(&references)
                .await?;
            println!("{count} ingrédient(s) de référence seedé(s).");
        }
    }
    Ok(())
}

/// Foyer cible : celui passé en option, sinon le foyer de démonstration.
fn resolve_household(explicit: Option<Uuid>) -> HouseholdId {
    HouseholdId::from(explicit.unwrap_or(DEMO_HOUSEHOLD_ID))
}

/// Clé d'upsert : titre normalisé (trim + minuscules) pour tolérer casse et
/// espaces superflus entre deux imports.
fn normalize(title: &str) -> String {
    title.trim().to_lowercase()
}

/// Importe les fichiers dans le foyer, en upsert par titre (idempotent).
async fn import_files(
    repo: &SqlxRecipeRepository,
    household: HouseholdId,
    paths: &[PathBuf],
) -> Result<usize> {
    // Index des recettes déjà présentes : un même titre est mis à jour plutôt
    // que dupliqué, ce qui rend un seed rejouable.
    let mut by_title: HashMap<String, RecipeId> = repo
        .list(household)
        .await?
        .iter()
        .map(|recipe| (normalize(&recipe.title), recipe.id))
        .collect();

    let mut count = 0;
    for path in paths {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("lecture de {}", path.display()))?;
        let doc: RecipeYaml = serde_yaml::from_str(&raw)
            .with_context(|| format!("YAML invalide : {}", path.display()))?;
        let key = normalize(&doc.title);

        if let Some(&id) = by_title.get(&key) {
            let recipe = doc.into_recipe(id, household)?;
            repo.update(&recipe).await?;
        } else {
            let id = RecipeId::new();
            let recipe = doc.into_recipe(id, household)?;
            repo.create(&recipe).await?;
            by_title.insert(key, id);
        }
        count += 1;
    }
    Ok(count)
}

/// Exporte les recettes du foyer : vers un dossier (un fichier par recette) ou
/// sur stdout (documents séparés par `---`).
async fn export(
    repo: &SqlxRecipeRepository,
    household: HouseholdId,
    out: Option<&Path>,
) -> Result<()> {
    let recipes = repo.list(household).await?;

    match out {
        None => {
            for (index, recipe) in recipes.iter().enumerate() {
                if index > 0 {
                    println!("---");
                }
                print!(
                    "{}",
                    serde_yaml::to_string(&RecipeYaml::from_recipe(recipe))?
                );
            }
        }
        Some(dir) => {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("création de {}", dir.display()))?;
            for recipe in &recipes {
                let path = dir.join(format!("{}.yaml", slug(&recipe.title)));
                let yaml = serde_yaml::to_string(&RecipeYaml::from_recipe(recipe))?;
                std::fs::write(&path, yaml)
                    .with_context(|| format!("écriture de {}", path.display()))?;
            }
            println!(
                "{} recette(s) exportée(s) dans {}",
                recipes.len(),
                dir.display()
            );
        }
    }
    Ok(())
}

/// Liste les fichiers `.yaml` / `.yml` d'un dossier, triés par nom (ordre stable).
fn yaml_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("lecture du dossier {}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| {
            matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("yaml" | "yml")
            )
        })
        .collect();
    paths.sort();
    Ok(paths)
}

/// Transforme un titre en nom de fichier : minuscules, alphanumérique et tirets.
fn slug(title: &str) -> String {
    let mut slug = String::with_capacity(title.len());
    let mut prev_dash = false;
    for ch in title.trim().to_lowercase().chars() {
        if ch.is_alphanumeric() {
            slug.push(ch);
            prev_dash = false;
        } else if !prev_dash {
            slug.push('-');
            prev_dash = true;
        }
    }
    let trimmed = slug.trim_matches('-');
    if trimmed.is_empty() {
        "recette".to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_title_for_idempotent_matching() {
        assert_eq!(normalize("  Ratatouille  "), normalize("ratatouille"));
    }

    #[test]
    fn slugifies_accented_titles() {
        assert_eq!(slug("Curry de courgettes !"), "curry-de-courgettes");
        assert_eq!(slug("Bœuf   bourguignon"), "bœuf-bourguignon");
    }

    #[test]
    fn slug_falls_back_when_empty() {
        assert_eq!(slug("!!!"), "recette");
    }
}
