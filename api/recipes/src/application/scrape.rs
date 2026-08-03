//! Use case d'import d'une recette par URL (#61).
//!
//! Récupère la page via le port [`RecipeScraper`] et en tire un brouillon à
//! relire dans le formulaire. Ne persiste rien **de la recette** : le champ URL
//! prérempli un formulaire que l'utilisateur corrige avant d'enregistrer.
//!
//! ## Rapatriement de la photo
//! Seule exception à ce « rien ne sort » : la photo. Le brouillon la désigne par
//! l'URL du site d'origine, qui ne tiendra pas — une image déplacée, et la fiche
//! affiche un cadre vide ; côté Instagram c'est certain, le CDN signe ses URLs
//! avec une date d'expiration. Elle est donc téléchargée et rangée dans le
//! stockage photo du foyer, comme le ferait un upload manuel, pour que la
//! recette garde une URL stable.
//!
//! Le rapatriement est **best-effort** : stockage non configuré, image
//! injoignable, trop lourde ou d'un format non pris en charge, et le brouillon
//! repart avec l'URL distante. Une photo qui expirera vaut mieux que pas de
//! photo, et l'import ne doit jamais échouer à cause d'elle.

use crate::domain::{ImageFetcher, PhotoStorage, RecipeScraper, ScrapeError, ScrapedRecipe};

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
    /// De quoi rapatrier la photo : le récupérateur d'images et le stockage.
    /// `None` quand l'appelant n'en a pas (CLI, ou API sans stockage photo
    /// configuré) — le brouillon garde alors l'URL distante.
    photos: Option<(&'a dyn ImageFetcher, &'a dyn PhotoStorage)>,
}

impl<'a> ScrapeRecipeHandler<'a> {
    /// Construit le handler. Sans rapatriement de photo : voir
    /// [`Self::with_photo_import`].
    #[must_use]
    pub fn new(scraper: &'a dyn RecipeScraper) -> Self {
        Self {
            scraper,
            photos: None,
        }
    }

    /// Active le rapatriement de la photo du brouillon dans le stockage.
    #[must_use]
    pub fn with_photo_import(
        mut self,
        images: &'a dyn ImageFetcher,
        storage: &'a dyn PhotoStorage,
    ) -> Self {
        self.photos = Some((images, storage));
        self
    }

    /// Exécute l'import. Ne renvoie jamais d'erreur : tout échec devient une
    /// [`ScrapeRecipeResponse::Rejected`] porteuse d'un [`ScrapeError`].
    pub async fn handle(&self, command: ScrapeRecipeCommand) -> ScrapeRecipeResponse {
        let url = command.url.trim();
        if url.is_empty() {
            return ScrapeRecipeResponse::Rejected(ScrapeError::InvalidUrl);
        }
        match self.scraper.scrape(url).await {
            Ok(mut recipe) => {
                recipe.photo = self.localize_photo(recipe.photo).await;
                ScrapeRecipeResponse::Drafted(recipe)
            }
            Err(error) => ScrapeRecipeResponse::Rejected(error),
        }
    }

    /// Télécharge la photo distante et renvoie l'URL du stockage. Rend l'URL
    /// d'origine à la moindre anicroche (cf. l'en-tête du module).
    async fn localize_photo(&self, remote: Option<String>) -> Option<String> {
        let remote = remote?;
        let Some((images, storage)) = self.photos else {
            return Some(remote);
        };
        let Ok(image) = images.fetch_image(&remote).await else {
            return Some(remote);
        };
        match storage.store_bytes(&image.bytes, &image.content_type).await {
            Ok(stored) => Some(stored),
            Err(_) => Some(remote),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{FetchedImage, PhotoError, PhotoUpload, ScrapedIngredient};
    use kernel::Unit;
    use std::sync::Mutex;

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
            servings: 2,
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

    // --- Rapatriement de la photo -----------------------------------------

    /// Récupérateur d'images de test : rend des octets figés, ou échoue.
    struct FakeImages(Option<FetchedImage>);

    #[async_trait::async_trait]
    impl ImageFetcher for FakeImages {
        async fn fetch_image(&self, _url: &str) -> Result<FetchedImage, ScrapeError> {
            self.0.clone().ok_or(ScrapeError::Unreachable)
        }
    }

    /// Stockage de test : mémorise ce qu'on lui range, ou refuse.
    #[derive(Default)]
    struct FakeStorage {
        stored: Mutex<Vec<(Vec<u8>, String)>>,
        refuse: bool,
    }

    #[async_trait::async_trait]
    impl PhotoStorage for FakeStorage {
        async fn presign_upload(&self, _content_type: &str) -> Result<PhotoUpload, PhotoError> {
            unimplemented!("hors sujet pour ces tests")
        }

        async fn store_bytes(
            &self,
            bytes: &[u8],
            content_type: &str,
        ) -> Result<String, PhotoError> {
            if self.refuse {
                return Err(PhotoError::Backend("stockage plein".to_owned()));
            }
            self.stored
                .lock()
                .unwrap()
                .push((bytes.to_vec(), content_type.to_owned()));
            Ok("https://week-meals.test/photos/abc.jpg".to_owned())
        }
    }

    fn with_remote_photo() -> ScrapedRecipe {
        ScrapedRecipe {
            photo: Some("https://cdn.instagram.test/signee.jpg?oe=65F0".to_owned()),
            ..sample()
        }
    }

    async fn draft_with(images: &dyn ImageFetcher, storage: &dyn PhotoStorage) -> ScrapedRecipe {
        let scraper = FakeScraper(Ok(with_remote_photo()));
        let response = ScrapeRecipeHandler::new(&scraper)
            .with_photo_import(images, storage)
            .handle(ScrapeRecipeCommand {
                url: "https://instagram.test/p/abcdef".to_owned(),
            })
            .await;
        match response {
            ScrapeRecipeResponse::Drafted(recipe) => recipe,
            other => panic!("attendu Drafted, obtenu {other:?}"),
        }
    }

    #[tokio::test]
    async fn stores_the_remote_photo_and_points_the_draft_at_it() {
        let images = FakeImages(Some(FetchedImage {
            bytes: vec![0xFF, 0xD8, 0xFF, 0x42],
            content_type: "image/jpeg".to_owned(),
        }));
        let storage = FakeStorage::default();
        let recipe = draft_with(&images, &storage).await;

        assert_eq!(
            recipe.photo.as_deref(),
            Some("https://week-meals.test/photos/abc.jpg")
        );
        let stored = storage.stored.lock().unwrap();
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].0, vec![0xFF, 0xD8, 0xFF, 0x42]);
        assert_eq!(stored[0].1, "image/jpeg");
    }

    #[tokio::test]
    async fn keeps_the_remote_url_when_the_image_cannot_be_fetched() {
        let recipe = draft_with(&FakeImages(None), &FakeStorage::default()).await;
        assert_eq!(
            recipe.photo.as_deref(),
            Some("https://cdn.instagram.test/signee.jpg?oe=65F0")
        );
    }

    #[tokio::test]
    async fn keeps_the_remote_url_when_the_storage_refuses() {
        let images = FakeImages(Some(FetchedImage {
            bytes: vec![0xFF, 0xD8, 0xFF],
            content_type: "image/jpeg".to_owned(),
        }));
        let storage = FakeStorage {
            refuse: true,
            ..FakeStorage::default()
        };
        let recipe = draft_with(&images, &storage).await;
        assert_eq!(
            recipe.photo.as_deref(),
            Some("https://cdn.instagram.test/signee.jpg?oe=65F0")
        );
    }

    #[tokio::test]
    async fn leaves_the_draft_alone_without_photo_import() {
        // Sans stockage configuré (ou en CLI), l'URL distante reste telle quelle.
        let scraper = FakeScraper(Ok(with_remote_photo()));
        let response = ScrapeRecipeHandler::new(&scraper)
            .handle(ScrapeRecipeCommand {
                url: "https://instagram.test/p/abcdef".to_owned(),
            })
            .await;
        match response {
            ScrapeRecipeResponse::Drafted(recipe) => assert_eq!(
                recipe.photo.as_deref(),
                Some("https://cdn.instagram.test/signee.jpg?oe=65F0")
            ),
            other => panic!("attendu Drafted, obtenu {other:?}"),
        }
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
