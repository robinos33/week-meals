//! Use case **Login** : vérifie pseudo + mot de passe, renvoie l'identité à
//! établir en session. Le Handler renvoie toujours un [`LoginResponse`], jamais
//! une exception (les échecs — mauvais identifiants, panne de repo — sont des
//! variantes de la réponse).

use kernel::{HouseholdId, UserId};

use crate::domain::password::{Password, PasswordHasher};
use crate::domain::user::Username;
use crate::domain::UserRepository;

/// Command de login : identifiants bruts issus de la présentation.
#[derive(Debug, Clone)]
pub struct LoginCommand {
    /// Pseudo saisi.
    pub username: String,
    /// Mot de passe saisi (en clair, jamais loggé).
    pub password: String,
}

/// Résultat d'un login. Le succès porte l'identité à mettre en session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginResponse {
    /// Identifiants valides : session à établir.
    Success {
        /// Utilisateur authentifié.
        user_id: UserId,
        /// Foyer de l'utilisateur (scope de toutes ses données).
        household_id: HouseholdId,
        /// Pseudo canonique.
        username: String,
    },
    /// Pseudo inconnu ou mot de passe erroné (indistincts, volontairement).
    InvalidCredentials,
    /// Panne technique (repo indisponible, hash illisible) → 500 côté présentation.
    Unavailable,
}

/// Handler du login : dépend d'un [`UserRepository`] et d'un
/// [`PasswordHasher`] (ports du domaine).
pub struct LoginHandler<'a> {
    users: &'a dyn UserRepository,
    hasher: &'a dyn PasswordHasher,
}

impl<'a> LoginHandler<'a> {
    /// Construit le handler à partir de ses ports.
    #[must_use]
    pub fn new(users: &'a dyn UserRepository, hasher: &'a dyn PasswordHasher) -> Self {
        Self { users, hasher }
    }

    /// Exécute le login. Ne renvoie jamais d'erreur : tout échec est encodé
    /// dans le [`LoginResponse`].
    pub async fn handle(&self, command: LoginCommand) -> LoginResponse {
        // Identifiants mal formés ⇒ indistinguables d'un mauvais couple.
        let Ok(username) = Username::new(command.username) else {
            return LoginResponse::InvalidCredentials;
        };
        let Ok(password) = Password::new(command.password) else {
            return LoginResponse::InvalidCredentials;
        };

        let user = match self.users.find_by_username(&username).await {
            Ok(Some(user)) => user,
            Ok(None) => return LoginResponse::InvalidCredentials,
            Err(_) => return LoginResponse::Unavailable,
        };

        match self.hasher.verify(&password, &user.password_hash) {
            Ok(true) => LoginResponse::Success {
                user_id: user.id,
                household_id: user.household_id,
                username: user.username.as_str().to_owned(),
            },
            Ok(false) => LoginResponse::InvalidCredentials,
            Err(_) => LoginResponse::Unavailable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::household::HouseholdName;
    use crate::domain::password::{Argon2Hasher, Password, PasswordHasher};
    use crate::domain::user::User;
    use crate::domain::{Household, UserRepository};
    use kernel::{HouseholdId, RepositoryError, UserId};

    /// Repo en mémoire pour tester le use case sans base.
    struct InMemoryUsers {
        users: Vec<User>,
        fail: bool,
    }

    #[async_trait::async_trait]
    impl UserRepository for InMemoryUsers {
        async fn create(&self, _user: &User) -> Result<(), RepositoryError> {
            Ok(())
        }
        async fn find(&self, id: UserId) -> Result<Option<User>, RepositoryError> {
            if self.fail {
                return Err(RepositoryError::Backend("down".into()));
            }
            Ok(self.users.iter().find(|u| u.id == id).cloned())
        }
        async fn find_by_username(
            &self,
            username: &Username,
        ) -> Result<Option<User>, RepositoryError> {
            if self.fail {
                return Err(RepositoryError::Backend("down".into()));
            }
            Ok(self
                .users
                .iter()
                .find(|u| u.username.as_str() == username.as_str())
                .cloned())
        }
    }

    fn user_with_password(username: &str, password: &str) -> (User, HouseholdId) {
        let household = Household::new(HouseholdName::new("Foyer").unwrap());
        let hash = Argon2Hasher::new()
            .hash(&Password::new(password).unwrap())
            .unwrap();
        let user = User::new(household.id, Username::new(username).unwrap(), hash);
        (user, household.id)
    }

    #[tokio::test]
    async fn valid_credentials_yield_success() {
        let (user, household_id) = user_with_password("robin", "correct horse");
        let user_id = user.id;
        let repo = InMemoryUsers {
            users: vec![user],
            fail: false,
        };
        let hasher = Argon2Hasher::new();
        let handler = LoginHandler::new(&repo, &hasher);

        let response = handler
            .handle(LoginCommand {
                username: "robin".into(),
                password: "correct horse".into(),
            })
            .await;

        assert_eq!(
            response,
            LoginResponse::Success {
                user_id,
                household_id,
                username: "robin".into(),
            }
        );
    }

    #[tokio::test]
    async fn wrong_password_is_invalid_credentials() {
        let (user, _) = user_with_password("robin", "correct horse");
        let repo = InMemoryUsers {
            users: vec![user],
            fail: false,
        };
        let hasher = Argon2Hasher::new();
        let handler = LoginHandler::new(&repo, &hasher);

        let response = handler
            .handle(LoginCommand {
                username: "robin".into(),
                password: "wrong donkey".into(),
            })
            .await;

        assert_eq!(response, LoginResponse::InvalidCredentials);
    }

    #[tokio::test]
    async fn unknown_user_is_invalid_credentials() {
        let repo = InMemoryUsers {
            users: vec![],
            fail: false,
        };
        let hasher = Argon2Hasher::new();
        let handler = LoginHandler::new(&repo, &hasher);

        let response = handler
            .handle(LoginCommand {
                username: "ghost".into(),
                password: "some password".into(),
            })
            .await;

        assert_eq!(response, LoginResponse::InvalidCredentials);
    }

    #[tokio::test]
    async fn repository_failure_is_unavailable() {
        let repo = InMemoryUsers {
            users: vec![],
            fail: true,
        };
        let hasher = Argon2Hasher::new();
        let handler = LoginHandler::new(&repo, &hasher);

        let response = handler
            .handle(LoginCommand {
                username: "robin".into(),
                password: "some password".into(),
            })
            .await;

        assert_eq!(response, LoginResponse::Unavailable);
    }

    #[tokio::test]
    async fn blank_username_is_invalid_credentials() {
        let repo = InMemoryUsers {
            users: vec![],
            fail: false,
        };
        let hasher = Argon2Hasher::new();
        let handler = LoginHandler::new(&repo, &hasher);

        let response = handler
            .handle(LoginCommand {
                username: "  ".into(),
                password: "some password".into(),
            })
            .await;

        assert_eq!(response, LoginResponse::InvalidCredentials);
    }
}
