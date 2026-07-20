//! Code d'appairage et hachage **Argon2id**.
//!
//! Le code d'appairage (cf. ADR-0006) est un secret **court-vivant** imprimé par
//! `weekmeals device open-window`, saisi une seule fois sur le téléphone pour
//! enrôler sa passkey. Seul son **hash** est stocké (colonne
//! `households.onboarding_code_hash`), et les tentatives sont comptées : la
//! fenêtre se referme au-delà de [`MAX_ONBOARDING_ATTEMPTS`](super::device::MAX_ONBOARDING_ATTEMPTS).
//!
//! Le hachage est du calcul **pur** (aucune I/O) : c'est un service de domaine.

use argon2::password_hash::{rand_core::OsRng, PasswordHash as PhcHash, SaltString};
use argon2::{Argon2, PasswordHasher as _, PasswordVerifier as _};
use password_hash::rand_core::RngCore;

/// Longueur du code d'appairage (hors tiret de mise en forme).
const CODE_LEN: usize = 8;

/// Alphabet du code : sans caractères visuellement ambigus (`0/O`, `1/I/L`).
/// 32 symboles → ~5 bits par caractère, ~40 bits sur 8 caractères.
const ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";

/// Code d'appairage en clair, sous sa forme **canonique** (majuscules, sans
/// séparateur). Masqué dans `Debug` : on ne l'expose jamais dans les logs.
#[derive(Clone, PartialEq, Eq)]
pub struct PairingCode(String);

impl PairingCode {
    /// Génère un code aléatoire (CSPRNG) de [`CODE_LEN`] caractères.
    #[must_use]
    pub fn generate() -> Self {
        let mut code = String::with_capacity(CODE_LEN);
        for _ in 0..CODE_LEN {
            // Rejet du biais modulo négligeable : 256 % 32 == 0, distribution uniforme.
            let idx = (OsRng.next_u32() as usize) % ALPHABET.len();
            code.push(ALPHABET[idx] as char);
        }
        Self(code)
    }

    /// Normalise puis valide une saisie (tirets/espaces retirés, majuscules).
    ///
    /// Renvoie `None` si la longueur ou l'alphabet ne correspond pas — traité
    /// comme un simple échec de code côté application.
    #[must_use]
    pub fn parse(input: &str) -> Option<Self> {
        let normalized: String = input
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_ascii_uppercase())
            .collect();
        if normalized.len() == CODE_LEN && normalized.bytes().all(|b| ALPHABET.contains(&b)) {
            Some(Self(normalized))
        } else {
            None
        }
    }

    /// Le code sous sa forme canonique (pour le hachage / la vérification).
    #[must_use]
    pub fn as_canonical(&self) -> &str {
        &self.0
    }

    /// Le code mis en forme pour l'affichage : `XXXX-XXXX`.
    #[must_use]
    pub fn formatted(&self) -> String {
        format!("{}-{}", &self.0[..4], &self.0[4..])
    }
}

impl std::fmt::Debug for PairingCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PairingCode(***)")
    }
}

/// Hash PHC d'un code d'appairage (chaîne `$argon2id$...`). Sûr à persister.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingCodeHash(String);

impl PairingCodeHash {
    /// Reconstitue un hash depuis la persistance.
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

/// Erreur de hachage d'un code d'appairage.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PairingError {
    /// Le hachage a échoué (erreur interne du KDF).
    #[error("pairing code hashing failed: {0}")]
    Hashing(String),
    /// La chaîne PHC stockée est malformée : impossible de vérifier.
    #[error("stored pairing code hash is malformed")]
    MalformedHash,
}

/// Port de hachage du code d'appairage. Implémenté par [`Argon2PairingHasher`].
pub trait PairingHasher: Send + Sync {
    /// Hache un code (sel aléatoire) en une chaîne PHC.
    ///
    /// # Errors
    /// [`PairingError::Hashing`] si le KDF échoue.
    fn hash(&self, code: &PairingCode) -> Result<PairingCodeHash, PairingError>;

    /// Vérifie un code contre un hash stocké.
    ///
    /// Renvoie `Ok(false)` pour un code simplement erroné ; n'échoue
    /// ([`PairingError::MalformedHash`]) que si le hash stocké est invalide.
    ///
    /// # Errors
    /// [`PairingError::MalformedHash`] si la chaîne PHC ne parse pas.
    fn verify(&self, code: &PairingCode, hash: &PairingCodeHash) -> Result<bool, PairingError>;
}

/// Hacheur **Argon2id** avec paramètres par défaut (recommandés par l'OWASP).
/// Sans état, clonable.
#[derive(Debug, Clone, Default)]
pub struct Argon2PairingHasher;

impl Argon2PairingHasher {
    /// Construit un hacheur Argon2id.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl PairingHasher for Argon2PairingHasher {
    fn hash(&self, code: &PairingCode) -> Result<PairingCodeHash, PairingError> {
        let salt = SaltString::generate(&mut OsRng);
        let phc = Argon2::default()
            .hash_password(code.as_canonical().as_bytes(), &salt)
            .map_err(|e| PairingError::Hashing(e.to_string()))?;
        Ok(PairingCodeHash::from_phc(phc.to_string()))
    }

    fn verify(&self, code: &PairingCode, hash: &PairingCodeHash) -> Result<bool, PairingError> {
        let parsed = PhcHash::new(hash.as_str()).map_err(|_| PairingError::MalformedHash)?;
        match Argon2::default().verify_password(code.as_canonical().as_bytes(), &parsed) {
            Ok(()) => Ok(true),
            Err(_) => Ok(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_code_has_expected_shape() {
        let code = PairingCode::generate();
        assert_eq!(code.as_canonical().len(), CODE_LEN);
        assert!(code.as_canonical().bytes().all(|b| ALPHABET.contains(&b)));
        let formatted = code.formatted();
        assert_eq!(formatted.len(), CODE_LEN + 1);
        assert_eq!(formatted.as_bytes()[4], b'-');
    }

    #[test]
    fn parse_normalizes_separators_and_case() {
        let code = PairingCode::generate();
        let typed = format!(" {} ", code.formatted().to_lowercase());
        assert_eq!(
            PairingCode::parse(&typed).unwrap().as_canonical(),
            code.as_canonical()
        );
    }

    #[test]
    fn parse_rejects_wrong_length_or_alphabet() {
        assert!(PairingCode::parse("ABC").is_none());
        // `0`, `1`, `O`, `I` sont hors alphabet.
        assert!(PairingCode::parse("0000IIII").is_none());
    }

    #[test]
    fn debug_masks_the_code() {
        let rendered = format!("{:?}", PairingCode::generate());
        assert_eq!(rendered, "PairingCode(***)");
    }

    #[test]
    fn hash_then_verify_round_trips() {
        let hasher = Argon2PairingHasher::new();
        let code = PairingCode::generate();
        let hash = hasher.hash(&code).unwrap();
        assert!(hash.as_str().starts_with("$argon2id$"));
        assert!(hasher.verify(&code, &hash).unwrap());
    }

    #[test]
    fn wrong_code_is_rejected_without_error() {
        let hasher = Argon2PairingHasher::new();
        let hash = hasher.hash(&PairingCode::generate()).unwrap();
        let other = loop {
            let c = PairingCode::generate();
            if c.as_canonical() != hash.as_str() {
                break c;
            }
        };
        assert!(!hasher.verify(&other, &hash).unwrap());
    }

    #[test]
    fn malformed_stored_hash_is_reported() {
        let hasher = Argon2PairingHasher::new();
        let err = hasher
            .verify(
                &PairingCode::generate(),
                &PairingCodeHash::from_phc("not-a-phc"),
            )
            .unwrap_err();
        assert_eq!(err, PairingError::MalformedHash);
    }
}
