//! Actions d'authentification. Les cérémonies WebAuthn (enrôlement,
//! authentification découvrable) vivent en [`presentation`](crate::presentation),
//! au plus près de la session et de l'instance `Webauthn`. Ne reste ici que
//! [`logout`], sans logique métier propre.

pub mod logout;

pub use logout::{LogoutCommand, LogoutHandler, LogoutResponse};
