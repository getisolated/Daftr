//! Hachage des mots de passe maîtres des profils avec Argon2id (paramètres par défaut du
//! crate `argon2` : 19 Mio de mémoire, 2 passes, recommandés par l'OWASP).
//!
//! L'empreinte est une chaîne PHC (`$argon2id$v=19$m=…,t=…,p=…$sel$empreinte`) : elle porte
//! l'algorithme, ses paramètres et un sel aléatoire propre à chaque mot de passe. La
//! vérification relit ces paramètres : on peut les durcir plus tard sans casser les
//! empreintes existantes.

use app::ports::PasswordHasher;
use app::{AppError, AppResult};
use argon2::Argon2;
use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher as _, PasswordVerifier as _};

/// Hachage Argon2id salé.
#[derive(Debug, Default)]
pub struct Argon2Hasher;

impl PasswordHasher for Argon2Hasher {
    fn hash(&self, password: &str) -> AppResult<String> {
        Argon2::default()
            .hash_password(password.as_bytes())
            .map(|hash| hash.to_string())
            .map_err(AppError::storage)
    }

    /// Comparaison en temps constant, faite par `argon2`.
    fn verify(&self, password: &str, hash: &str) -> bool {
        let Ok(parsed) = PasswordHash::new(hash) else {
            return false;
        };
        Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_the_password_against_its_salted_hash() {
        let hash = Argon2Hasher.hash("correct horse").unwrap();

        assert!(hash.starts_with("$argon2id$"));
        assert!(!hash.contains("correct horse"));
        assert!(Argon2Hasher.verify("correct horse", &hash));
        assert!(!Argon2Hasher.verify("Correct horse", &hash));
        assert!(!Argon2Hasher.verify("", &hash));
    }

    #[test]
    fn salts_every_hash() {
        let first = Argon2Hasher.hash("same").unwrap();
        let second = Argon2Hasher.hash("same").unwrap();

        assert_ne!(first, second);
        assert!(Argon2Hasher.verify("same", &second));
    }

    #[test]
    fn an_unreadable_hash_matches_nothing() {
        assert!(!Argon2Hasher.verify("x", "not a hash"));
        assert!(!Argon2Hasher.verify("x", ""));
    }
}
