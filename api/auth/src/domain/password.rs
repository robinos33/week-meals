//! Value objects du mot de passe et service de hachage **Argon2id**.
//!
//! Le hachage est un **service de domaine pur** : il ne fait aucune I/O (juste
//! du calcul CPU), il est donc trivialement testable et vit dans le `domain` —
//! contrairement aux repositories, dont les implémentations sont en
//! infrastructure. Voir ADR-0002 (auth sans email, hash Argon2id).

use std::fmt;

use argon2::password_hash::{
    PasswordHash as PhcHash, PasswordHasher as _, PasswordVerifier as _, SaltString,
};
use argon2::Argon2;
use rand_core::OsRng;
use thiserror::Error;

/// Longueur minimale d'un mot de passe en clair (caractères Unicode).
pub const MIN_PASSWORD_LEN: usize = 8;
/// Longueur maximale d'un mot de passe en clair (borne la charge de calcul du
/// hachage et évite un déni de service par entrée démesurée).
pub const MAX_PASSWORD_LEN: usize = 128;

/// Mot de passe **en clair**, validé mais non encore haché.
///
/// Ne dérive pas `Debug`/`Display` de façon à ne jamais divulguer le secret
/// dans les logs ; l'implémentation manuelle de [`fmt::Debug`] le masque.
#[derive(Clone)]
pub struct Password(String);

impl Password {
    /// Valide et construit un mot de passe en clair.
    ///
    /// # Errors
    ///
    /// Renvoie [`PasswordError`] si la longueur est hors des bornes
    /// [`MIN_PASSWORD_LEN`]..=[`MAX_PASSWORD_LEN`].
    pub fn new(raw: impl Into<String>) -> Result<Self, PasswordError> {
        let raw = raw.into();
        let len = raw.chars().count();
        if len < MIN_PASSWORD_LEN {
            return Err(PasswordError::TooShort {
                min: MIN_PASSWORD_LEN,
            });
        }
        if len > MAX_PASSWORD_LEN {
            return Err(PasswordError::TooLong {
                max: MAX_PASSWORD_LEN,
            });
        }
        Ok(Self(raw))
    }

    /// Octets du mot de passe, à destination du hacheur.
    fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl fmt::Debug for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Password(***)")
    }
}

/// Erreurs de validation d'un [`Password`] en clair.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PasswordError {
    /// Mot de passe plus court que [`MIN_PASSWORD_LEN`].
    #[error("mot de passe trop court (minimum {min} caractères)")]
    TooShort {
        /// Longueur minimale requise.
        min: usize,
    },
    /// Mot de passe plus long que [`MAX_PASSWORD_LEN`].
    #[error("mot de passe trop long (maximum {max} caractères)")]
    TooLong {
        /// Longueur maximale autorisée.
        max: usize,
    },
}

/// Hash d'un mot de passe, sous forme de chaîne **PHC** (`$argon2id$...`).
///
/// C'est cette valeur — jamais le mot de passe en clair — qui est persistée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordHash(String);

impl PasswordHash {
    /// Reconstruit un hash depuis une chaîne PHC déjà stockée (persistance).
    #[must_use]
    pub fn from_phc(phc: impl Into<String>) -> Self {
        Self(phc.into())
    }

    /// La chaîne PHC sous-jacente.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Erreurs du service de hachage.
#[derive(Debug, Error)]
pub enum PasswordHashError {
    /// Échec lors du calcul du hash.
    #[error("échec du hachage : {0}")]
    Hashing(String),
    /// La chaîne PHC fournie à la vérification est malformée.
    #[error("hash stocké invalide : {0}")]
    Malformed(String),
    /// Défaillance technique lors de la vérification (hors « mauvais mot de
    /// passe », qui est un `Ok(false)`).
    #[error("échec de la vérification : {0}")]
    Verification(String),
}

/// Port de hachage de mot de passe.
///
/// Exposé comme trait pour que les use cases (inscription, connexion) dépendent
/// d'une abstraction et soient testables avec un double. L'implémentation par
/// défaut [`Argon2Hasher`] est un service *pur* et vit dans le domaine.
pub trait PasswordHasher {
    /// Hache un mot de passe en clair vers une chaîne PHC Argon2id (sel
    /// aléatoire).
    ///
    /// # Errors
    ///
    /// Renvoie [`PasswordHashError::Hashing`] si le calcul échoue.
    fn hash(&self, password: &Password) -> Result<PasswordHash, PasswordHashError>;

    /// Vérifie un mot de passe candidat contre un hash stocké.
    ///
    /// Renvoie `Ok(true)` si le mot de passe correspond, `Ok(false)` s'il ne
    /// correspond pas.
    ///
    /// # Errors
    ///
    /// Renvoie [`PasswordHashError::Malformed`] si le hash stocké est invalide,
    /// ou [`PasswordHashError::Verification`] en cas de défaillance technique.
    fn verify(&self, candidate: &Password, hash: &PasswordHash) -> Result<bool, PasswordHashError>;
}

/// Implémentation Argon2**id** avec les paramètres par défaut de la crate
/// `argon2` (recommandés par l'OWASP). Sans état : un sel aléatoire est tiré à
/// chaque hachage.
#[derive(Debug, Default, Clone, Copy)]
pub struct Argon2Hasher;

impl PasswordHasher for Argon2Hasher {
    fn hash(&self, password: &Password) -> Result<PasswordHash, PasswordHashError> {
        let salt = SaltString::generate(&mut OsRng);
        let phc = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| PasswordHashError::Hashing(e.to_string()))?
            .to_string();
        Ok(PasswordHash(phc))
    }

    fn verify(&self, candidate: &Password, hash: &PasswordHash) -> Result<bool, PasswordHashError> {
        let parsed =
            PhcHash::new(hash.as_str()).map_err(|e| PasswordHashError::Malformed(e.to_string()))?;
        match Argon2::default().verify_password(candidate.as_bytes(), &parsed) {
            Ok(()) => Ok(true),
            Err(argon2::password_hash::Error::Password) => Ok(false),
            Err(e) => Err(PasswordHashError::Verification(e.to_string())),
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
    fn rejette_un_mot_de_passe_trop_court() {
        assert!(matches!(
            Password::new("court"),
            Err(PasswordError::TooShort {
                min: MIN_PASSWORD_LEN
            })
        ));
    }

    #[test]
    fn rejette_un_mot_de_passe_trop_long() {
        let raw = "a".repeat(MAX_PASSWORD_LEN + 1);
        assert!(matches!(
            Password::new(raw),
            Err(PasswordError::TooLong { .. })
        ));
    }

    #[test]
    fn debug_ne_divulgue_pas_le_secret() {
        let rendered = format!("{:?}", pw("s3cret-de-ouf"));
        assert_eq!(rendered, "Password(***)");
    }

    #[test]
    fn hash_puis_verify_reussit_pour_le_bon_mot_de_passe() {
        let hasher = Argon2Hasher;
        let password = pw("correct horse battery");
        let hash = hasher.hash(&password).unwrap();
        assert!(hasher.verify(&password, &hash).unwrap());
    }

    #[test]
    fn verify_echoue_pour_un_mauvais_mot_de_passe() {
        let hasher = Argon2Hasher;
        let hash = hasher.hash(&pw("correct horse battery")).unwrap();
        assert!(!hasher.verify(&pw("Tr0ub4dour-et-3"), &hash).unwrap());
    }

    #[test]
    fn le_hash_est_argon2id_et_sale_aleatoirement() {
        let hasher = Argon2Hasher;
        let password = pw("correct horse battery");
        let h1 = hasher.hash(&password).unwrap();
        let h2 = hasher.hash(&password).unwrap();
        // Sel aléatoire → deux hash distincts pour le même mot de passe.
        assert_ne!(h1.as_str(), h2.as_str());
        // Variante Argon2id (préfixe de la chaîne PHC).
        assert!(h1.as_str().starts_with("$argon2id$"));
    }

    #[test]
    fn verify_signale_un_hash_malforme() {
        let hasher = Argon2Hasher;
        let bogus = PasswordHash::from_phc("pas-une-chaine-phc");
        assert!(matches!(
            hasher.verify(&pw("correct horse battery"), &bogus),
            Err(PasswordHashError::Malformed(_))
        ));
    }
}
