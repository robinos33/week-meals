//! Use case **Logout**. La déconnexion n'a pas de logique métier propre — elle
//! consiste à invalider la session côté présentation. Ce handler existe pour la
//! symétrie du pattern Command/Handler/Response et pour offrir un point
//! d'extension (audit, révocation de tokens…) sans toucher aux routes.

/// Command de logout : aucune donnée requise (l'identité vient de la session).
#[derive(Debug, Clone, Default)]
pub struct LogoutCommand;

/// Résultat d'un logout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogoutResponse {
    /// Session à invalider côté présentation.
    Success,
}

/// Handler du logout.
#[derive(Debug, Clone, Default)]
pub struct LogoutHandler;

impl LogoutHandler {
    /// Construit le handler.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Exécute le logout. La présentation doit ensuite vider la session.
    #[must_use]
    pub fn handle(&self, _command: LogoutCommand) -> LogoutResponse {
        LogoutResponse::Success
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logout_always_succeeds() {
        assert_eq!(
            LogoutHandler::new().handle(LogoutCommand),
            LogoutResponse::Success
        );
    }
}
