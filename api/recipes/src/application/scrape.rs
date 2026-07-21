//! Use case d'import d'une recette par URL (#61).
//!
//! Récupère la page via le port [`RecipeScraper`] et en tire un brouillon à
//! relire dans le formulaire. Ne persiste rien : le champ URL prérempli un
//! formulaire que l'utilisateur corrige avant d'enregistrer.

use crate::domain::{RecipeScraper, ScrapeError, ScrapedRecipe};

/// Command : l'URL à importer.
#[derive(Debug, Clone)]
pub struct ScrapeRecipeCommand {
    /// URL de la page de recette.
    pub url: String,
}

/// Résultat d'un import par URL.
#[derive(Debug)]
pub enum ScrapeRecipeResponse {
    /// Brouillon extrait, à relire dans le formulaire.
    Drafted(ScrapedRecipe),
    /// Import refusé (URL invalide, cible interdite, page sans recette…). La
    /// présentation en tire un message ; le front garde la saisie en cours.
    Rejected(ScrapeError),
}

/// Handler d'import par URL.
pub struct ScrapeRecipeHandler<'a> {
    scraper: &'a dyn RecipeScraper,
}

impl<'a> ScrapeRecipeHandler<'a> {
    /// Construit le handler.
    #[must_use]
    pub fn new(scraper: &'a dyn RecipeScraper) -> Self {
        Self { scraper }
    }

    /// Exécute l'import. Ne renvoie jamais d'erreur : tout échec devient une
    /// [`ScrapeRecipeResponse::Rejected`] porteuse d'un [`ScrapeError`].
    pub async fn handle(&self, command: ScrapeRecipeCommand) -> ScrapeRecipeResponse {
        let url = command.url.trim();
        if url.is_empty() {
            return ScrapeRecipeResponse::Rejected(ScrapeError::InvalidUrl);
        }
        match self.scraper.scrape(url).await {
            Ok(recipe) => ScrapeRecipeResponse::Drafted(recipe),
            Err(error) => ScrapeRecipeResponse::Rejected(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ScrapedIngredient;
    use kernel::Unit;

    /// Scraper de test : renvoie un résultat figé sans réseau.
    struct FakeScraper(Result<ScrapedRecipe, ScrapeError>);

    #[async_trait::async_trait]
    impl RecipeScraper for FakeScraper {
        async fn scrape(&self, _url: &str) -> Result<ScrapedRecipe, ScrapeError> {
            match &self.0 {
                Ok(recipe) => Ok(recipe.clone()),
                Err(_) => Err(ScrapeError::NoRecipe),
            }
        }
    }

    fn sample() -> ScrapedRecipe {
        ScrapedRecipe {
            title: "Ratatouille".to_owned(),
            prep_time_min: Some(25),
            cook_time_min: Some(45),
            photo: None,
            ingredients: vec![ScrapedIngredient {
                name: "courgette".to_owned(),
                amount: 600.0,
                unit: Unit::G,
            }],
            steps: vec!["Émincer.".to_owned()],
        }
    }

    #[tokio::test]
    async fn drafts_a_scraped_recipe() {
        let scraper = FakeScraper(Ok(sample()));
        let response = ScrapeRecipeHandler::new(&scraper)
            .handle(ScrapeRecipeCommand {
                url: "https://example.test/rata".to_owned(),
            })
            .await;
        match response {
            ScrapeRecipeResponse::Drafted(recipe) => assert_eq!(recipe.title, "Ratatouille"),
            other => panic!("attendu Drafted, obtenu {other:?}"),
        }
    }

    #[tokio::test]
    async fn rejects_a_blank_url_without_calling_the_scraper() {
        let scraper = FakeScraper(Ok(sample()));
        let response = ScrapeRecipeHandler::new(&scraper)
            .handle(ScrapeRecipeCommand {
                url: "   ".to_owned(),
            })
            .await;
        assert!(matches!(
            response,
            ScrapeRecipeResponse::Rejected(ScrapeError::InvalidUrl)
        ));
    }

    #[tokio::test]
    async fn surfaces_a_scraper_error() {
        let scraper = FakeScraper(Err(ScrapeError::NoRecipe));
        let response = ScrapeRecipeHandler::new(&scraper)
            .handle(ScrapeRecipeCommand {
                url: "https://example.test/page".to_owned(),
            })
            .await;
        assert!(matches!(
            response,
            ScrapeRecipeResponse::Rejected(ScrapeError::NoRecipe)
        ));
    }
}
