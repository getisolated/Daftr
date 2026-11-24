//! Identité de l'appareil (clé statique x25519 de Noise) et empreinte de sa clé publique.

use std::fmt;

use app::ports::trust::{DeviceSecret, TrustStore};
use domain::sync::DeviceKey;
use sha2::{Digest, Sha256};
use snow::params::DHChoice;
use snow::resolvers::{CryptoResolver, DefaultResolver};

use crate::error::{Error, Result};
use crate::noise::NOISE_PARAMS;

const KEY_BYTES: usize = 32;
const FINGERPRINT_BYTES: usize = 16;

/// Clé statique de l'appareil : la clé privée (`device.key`) et la clé publique qui en
/// découle, identifiant de l'appareil.
#[derive(Clone)]
pub struct DeviceIdentity {
    secret: DeviceSecret,
    key: DeviceKey,
}

impl DeviceIdentity {
    /// Tire une nouvelle clé.
    pub fn generate() -> Result<Self> {
        let pair = snow::Builder::new(NOISE_PARAMS.parse()?).generate_keypair()?;
        let secret = to_array(&pair.private).ok_or(Error::Malformed)?;
        Self::from_secret(DeviceSecret::from_bytes(secret))
    }

    /// L'identité d'une clé privée enregistrée (clé publique recalculée).
    pub fn from_secret(secret: DeviceSecret) -> Result<Self> {
        let mut dh = DefaultResolver
            .resolve_dh(&DHChoice::Curve25519)
            .ok_or_else(|| Error::Noise("x25519 unavailable".to_owned()))?;
        dh.set(secret.as_bytes());
        let key = to_array(dh.pubkey()).ok_or(Error::Malformed)?;
        Ok(Self {
            secret,
            key: DeviceKey::from_bytes(key),
        })
    }

    /// Clé publique : l'identifiant de l'appareil.
    pub fn key(&self) -> DeviceKey {
        self.key
    }

    /// Clé privée.
    pub fn secret(&self) -> &DeviceSecret {
        &self.secret
    }

    /// Empreinte publiable de la clé publique.
    pub fn fingerprint(&self) -> Fingerprint {
        Fingerprint::of(&self.key)
    }
}

impl fmt::Debug for DeviceIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "DeviceIdentity({})", self.key)
    }
}

/// L'identité enregistrée de l'appareil, créée et enregistrée au premier appel.
pub fn load_or_create_identity(trust: &dyn TrustStore) -> Result<DeviceIdentity> {
    if let Some(secret) = trust.device_secret()? {
        return DeviceIdentity::from_secret(secret);
    }
    let identity = DeviceIdentity::generate()?;
    trust.save_device_secret(identity.secret())?;
    Ok(identity)
}

fn to_array(bytes: &[u8]) -> Option<[u8; KEY_BYTES]> {
    bytes.try_into().ok()
}

/// Empreinte d'une clé publique : les 16 premiers octets de son SHA-256, écrits en 32
/// chiffres hexadécimaux minuscules. Publiée dans l'annonce mDNS et le QR code.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Fingerprint([u8; FINGERPRINT_BYTES]);

impl Fingerprint {
    /// L'empreinte de `key`.
    pub fn of(key: &DeviceKey) -> Self {
        let digest = Sha256::digest(key.as_bytes());
        let mut bytes = [0_u8; FINGERPRINT_BYTES];
        bytes.copy_from_slice(&digest[..FINGERPRINT_BYTES]);
        Self(bytes)
    }

    /// L'empreinte écrite en 32 chiffres hexadécimaux (majuscules admises), `None` sinon.
    pub fn parse(text: &str) -> Option<Self> {
        if text.len() != 2 * FINGERPRINT_BYTES || !text.is_ascii() {
            return None;
        }
        let mut bytes = [0_u8; FINGERPRINT_BYTES];
        for (byte, pair) in bytes.iter_mut().zip(text.as_bytes().chunks(2)) {
            let pair = std::str::from_utf8(pair).ok()?;
            *byte = u8::from_str_radix(pair, 16).ok()?;
        }
        Some(Self(bytes))
    }

    /// `key` a cette empreinte.
    pub fn matches(&self, key: &DeviceKey) -> bool {
        Self::of(key) == *self
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0
            .iter()
            .try_for_each(|byte| write!(formatter, "{byte:02x}"))
    }
}

impl fmt::Debug for Fingerprint {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Fingerprint({self})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use app::sync::testing::MemoryTrust;

    #[test]
    fn the_public_key_is_derived_from_the_secret() {
        let identity = DeviceIdentity::generate().unwrap();
        let reloaded = DeviceIdentity::from_secret(identity.secret().clone()).unwrap();
        assert_eq!(reloaded.key(), identity.key());
        assert_ne!(DeviceIdentity::generate().unwrap().key(), identity.key());
    }

    #[test]
    fn the_identity_is_created_once() {
        let trust = MemoryTrust::default();
        let first = load_or_create_identity(&trust).unwrap();
        let second = load_or_create_identity(&trust).unwrap();
        assert_eq!(first.key(), second.key());
    }

    #[test]
    fn fingerprints_round_trip_through_hex() {
        let key = DeviceKey::from_bytes([3; 32]);
        let fingerprint = Fingerprint::of(&key);
        let text = fingerprint.to_string();
        assert_eq!(text.len(), 32);
        assert_eq!(Fingerprint::parse(&text), Some(fingerprint));
        assert_eq!(Fingerprint::parse(&text.to_uppercase()), Some(fingerprint));
        assert!(fingerprint.matches(&key));
        assert!(!fingerprint.matches(&DeviceKey::from_bytes([4; 32])));
        for bad in ["", "zz", &text[..30], &format!("{text}00"), &"é".repeat(16)] {
            assert_eq!(Fingerprint::parse(bad), None, "{bad}");
        }
    }
}
