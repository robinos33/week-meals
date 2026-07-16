//! Mot de passe et hachage **Argon2id**.
//!
//! Le hachage est du calcul **pur** (aucune I/O) : c'est donc un service de
//! domaine, pas d'infrastructure (cf. ADR-0002). Le secret en clair
//! ([`Password`]) est masqué dans `Debug` pour ne jamais fuiter dans les logs.

use argon2::password_hash::{rand_core::OsRng, PasswordHash as PhcHash, SaltString};
use argon2::{Argon2, PasswordHasher as _, PasswordVerifier as _};

use super::AuthError;

/// Longueur minimale d'un mot de passe.
const PASSWORD_MIN_LEN: usize = 8;

/// Mot de passe en clair, validé (longueur minimale). Le secret est masqué
/// dans `Debug` : on ne l'expose jamais dans les logs.
#[derive(Clone)]
pub struct Password(String);

impl Password {
    /// Construit un mot de passe valide.
    ///
    /// # Errors
    /// [`AuthError::PasswordTooShort`] en deçà de [`PASSWORD_MIN_LEN`]
    /// caractères. Le mot de passe n'est **pas** trimé (les espaces sont
    /// significatifs dans une phrase de passe).
    pub fn new(value: impl Into<String>) -> Result<Self, AuthError> {
        let value = value.into();
        if value.chars().count() < PASSWORD_MIN_LEN {
            return Err(AuthError::PasswordTooShort {
                min: PASSWORD_MIN_LEN,
            });
        }
        Ok(Self(value))
    }

    /// Les octets du secret (pour le hachage / la vérification).
    fn expose(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl std::fmt::Debug for Password {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Password(***)")
    }
}

/// Hash PHC d'un mot de passe (chaîne `$argon2id$...`). Sûr à persister.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordHash(String);

impl PasswordHash {
    /// Reconstitue un hash depuis la persistance. La validité de la chaîne PHC
    /// est vérifiée à la [`PasswordHasher::verify`], pas ici.
    #[must_use]
    pub fn from_phc(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// La chaîne PHC sous-jacente (pour la persistance).
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Erreur de hachage / vérification d'un mot de passe.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PasswordError {
    /// Le hachage a échoué (erreur interne du KDF).
    #[error("password hashing failed: {0}")]
    Hashing(String),
    /// La chaîne PHC stockée est malformée : impossible de vérifier.
    #[error("stored password hash is malformed")]
    MalformedHash,
}

/// Port de hachage : abstrait le KDF pour les use cases. Implémenté par
/// [`Argon2Hasher`] (pur, sans I/O).
pub trait PasswordHasher: Send + Sync {
    /// Hache un mot de passe (sel aléatoire) en une chaîne PHC.
    ///
    /// # Errors
    /// [`PasswordError::Hashing`] si le KDF échoue.
    fn hash(&self, password: &Password) -> Result<PasswordHash, PasswordError>;

    /// Vérifie un mot de passe contre un hash stocké.
    ///
    /// Renvoie `Ok(false)` pour un mot de passe simplement erroné ;
    /// n'échoue ([`PasswordError::MalformedHash`]) que si le hash stocké est
    /// invalide.
    ///
    /// # Errors
    /// [`PasswordError::MalformedHash`] si la chaîne PHC ne parse pas.
    fn verify(&self, password: &Password, hash: &PasswordHash) -> Result<bool, PasswordError>;
}

/// Hacheur **Argon2id** avec paramètres par défaut de la crate `argon2`
/// (recommandés par l'OWASP). Sans état, clonable.
#[derive(Debug, Clone, Default)]
pub struct Argon2Hasher;

impl Argon2Hasher {
    /// Construit un hacheur Argon2id.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl PasswordHasher for Argon2Hasher {
    fn hash(&self, password: &Password) -> Result<PasswordHash, PasswordError> {
        let salt = SaltString::generate(&mut OsRng);
        let phc = Argon2::default()
            .hash_password(password.expose(), &salt)
            .map_err(|e| PasswordError::Hashing(e.to_string()))?;
        Ok(PasswordHash::from_phc(phc.to_string()))
    }

    fn verify(&self, password: &Password, hash: &PasswordHash) -> Result<bool, PasswordError> {
        let parsed = PhcHash::new(hash.as_str()).map_err(|_| PasswordError::MalformedHash)?;
        match Argon2::default().verify_password(password.expose(), &parsed) {
            Ok(()) => Ok(true),
            Err(argon2::password_hash::Error::Password) => Ok(false),
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pw(s: &str) -> Password {
        Password::new(s).unwrap()
    }

    #[test]
    fn rejects_short_passwords() {
        assert_eq!(
            Password::new("short").unwrap_err(),
            AuthError::PasswordTooShort {
                min: PASSWORD_MIN_LEN
            }
        );
    }

    #[test]
    fn debug_masks_the_secret() {
        let rendered = format!("{:?}", pw("supersecret"));
        assert_eq!(rendered, "Password(***)");
        assert!(!rendered.contains("supersecret"));
    }

    #[test]
    fn hash_then_verify_round_trips() {
        let hasher = Argon2Hasher::new();
        let hash = hasher.hash(&pw("correct horse")).unwrap();
        assert!(hash.as_str().starts_with("$argon2id$"));
        assert!(hasher.verify(&pw("correct horse"), &hash).unwrap());
    }

    #[test]
    fn wrong_password_is_rejected_without_error() {
        let hasher = Argon2Hasher::new();
        let hash = hasher.hash(&pw("correct horse")).unwrap();
        assert!(!hasher.verify(&pw("wrong donkey"), &hash).unwrap());
    }

    #[test]
    fn salt_is_random_between_hashes() {
        let hasher = Argon2Hasher::new();
        let a = hasher.hash(&pw("same password")).unwrap();
        let b = hasher.hash(&pw("same password")).unwrap();
        assert_ne!(a.as_str(), b.as_str());
    }

    #[test]
    fn malformed_stored_hash_is_reported() {
        let hasher = Argon2Hasher::new();
        let err = hasher
            .verify(
                &pw("whatever pass"),
                &PasswordHash::from_phc("not-a-phc-string"),
            )
            .unwrap_err();
        assert_eq!(err, PasswordError::MalformedHash);
    }
}
