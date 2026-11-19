//! Identité de l'appareil (clé statique Noise), empreinte et code d'appairage.

use rand::RngExt;
use sha2::{Digest, Sha256};

/// Paramètres Noise retenus par la spec (§1).
pub const NOISE_PARAMS: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

/// Clé statique x25519. Dans le spike elle est éphémère (régénérée à chaque lancement).
pub struct Identity {
    /// Clé privée (32 octets).
    pub private: Vec<u8>,
    /// Clé publique (32 octets), identifiant de l'appareil.
    pub public: Vec<u8>,
}

impl Identity {
    /// Génère une nouvelle paire de clés.
    pub fn generate() -> anyhow::Result<Self> {
        let builder = snow::Builder::new(NOISE_PARAMS.parse()?);
        let keypair = builder.generate_keypair()?;
        Ok(Self {
            private: keypair.private,
            public: keypair.public,
        })
    }

    /// Empreinte publiable de la clé publique.
    pub fn fingerprint(&self) -> String {
        fingerprint(&self.public)
    }

    /// Identifiant client Yjs dérivé de la clé (53 bits, compatible JavaScript).
    pub fn yjs_client_id(&self) -> u64 {
        let digest = Sha256::digest(&self.public);
        let mut bytes = [0u8; 8];
        bytes.copy_from_slice(&digest[..8]);
        u64::from_le_bytes(bytes) & ((1 << 53) - 1)
    }
}

/// Empreinte d'une clé publique : 16 premiers octets de SHA-256, en hexadécimal.
pub fn fingerprint(public_key: &[u8]) -> String {
    Sha256::digest(public_key)[..16]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Tire un code d'appairage à 6 chiffres.
pub fn random_pairing_code() -> String {
    format!("{:06}", rand::rng().random_range(0..1_000_000u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pairing_code_has_six_digits() {
        for _ in 0..100 {
            let code = random_pairing_code();
            assert_eq!(code.len(), 6);
            assert!(code.chars().all(|c| c.is_ascii_digit()));
        }
    }

    #[test]
    fn client_id_fits_53_bits() {
        let identity = Identity::generate().unwrap();
        assert!(identity.yjs_client_id() < (1 << 53));
        assert_eq!(identity.fingerprint().len(), 32);
    }
}
