//! Mode appairage de l'appareil qui affiche le code : code courant, échecs, expiration.

use std::time::{Duration, Instant};

use domain::sync::{DeviceKey, DeviceName, PairingCode};

use crate::error::{Error, Result};
use crate::qr::PairingUri;

/// Durée du mode appairage.
pub const PAIRING_DURATION: Duration = Duration::from_secs(5 * 60);
/// Échecs de vérification avant le tirage d'un nouveau code.
pub const MAX_FAILURES: u32 = 10;

/// Ce que l'interface affiche pendant le mode appairage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingTicket {
    /// Code à 6 chiffres.
    pub code: PairingCode,
    /// Contenu du QR code.
    pub uri: PairingUri,
    /// QR code rendu en SVG.
    pub qr_svg: String,
    /// Fin du mode (compte à rebours).
    pub expires_at: Instant,
}

/// État du mode appairage, suivi par l'interface (nouveau code après trop d'échecs, fin).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum PairingState {
    /// Pas de mode appairage en cours.
    #[default]
    Inactive,
    /// Mode appairage ouvert.
    Active(PairingTicket),
}

/// Appareil lié par un appairage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedDevice {
    /// Sa clé.
    pub key: DeviceKey,
    /// Son nom.
    pub name: DeviceName,
}

/// Mode appairage ouvert : code courant, échecs, expiration.
#[derive(Debug)]
pub(crate) struct PairingMode {
    code: PairingCode,
    failures: u32,
    expires_at: Instant,
}

impl PairingMode {
    /// Ouvre le mode à `now` pour `duration`.
    pub(crate) fn new(code: PairingCode, now: Instant, duration: Duration) -> Self {
        Self {
            code,
            failures: 0,
            expires_at: now + duration,
        }
    }

    /// Le mode accepte encore des tentatives à `now`.
    pub(crate) fn is_open(&self, now: Instant) -> bool {
        now < self.expires_at
    }

    /// Le code courant.
    pub(crate) fn code(&self) -> &PairingCode {
        &self.code
    }

    /// Fin du mode.
    pub(crate) fn expires_at(&self) -> Instant {
        self.expires_at
    }

    /// Compte un échec ; au [`MAX_FAILURES`]e, remplace le code par `fresh` et rend `true`.
    pub(crate) fn record_failure(&mut self, fresh: PairingCode) -> bool {
        self.failures += 1;
        if self.failures < MAX_FAILURES {
            return false;
        }
        self.failures = 0;
        self.code = fresh;
        true
    }
}

/// Tire un code à 6 chiffres uniformément (tirage rejeté au-delà du plus grand multiple de
/// 10⁶ représentable, pour ne pas favoriser les petits codes).
pub(crate) fn random_code() -> Result<PairingCode> {
    const LIMIT: u32 = u32::MAX - u32::MAX % 1_000_000;
    loop {
        let mut bytes = [0_u8; 4];
        getrandom::fill(&mut bytes).map_err(|error| Error::Noise(error.to_string()))?;
        let number = u32::from_le_bytes(bytes);
        if number < LIMIT {
            return PairingCode::from_number(number % 1_000_000).ok_or(Error::Malformed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code(text: &str) -> PairingCode {
        PairingCode::parse(text).unwrap()
    }

    #[test]
    fn the_mode_expires() {
        let now = Instant::now();
        let mode = PairingMode::new(code("123456"), now, PAIRING_DURATION);
        assert!(mode.is_open(now));
        assert!(mode.is_open(now + PAIRING_DURATION - Duration::from_secs(1)));
        assert!(!mode.is_open(now + PAIRING_DURATION));
    }

    #[test]
    fn ten_failures_renew_the_code() {
        let mut mode = PairingMode::new(code("123456"), Instant::now(), PAIRING_DURATION);
        for _ in 1..MAX_FAILURES {
            assert!(!mode.record_failure(code("654321")));
            assert_eq!(mode.code().as_str(), "123456");
        }
        assert!(mode.record_failure(code("654321")));
        assert_eq!(mode.code().as_str(), "654321");
        assert!(!mode.record_failure(code("000000")));
    }

    #[test]
    fn random_codes_have_six_digits() {
        for _ in 0..200 {
            assert_eq!(random_code().unwrap().as_str().len(), 6);
        }
    }
}
