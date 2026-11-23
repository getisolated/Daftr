//! Horloge injectable : instants pour les délais (mode appairage), horodatages pour les dates
//! écrites (ajout d'un appareil, dernier échange).

use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Source du temps du moteur.
pub trait Clock: Send + Sync {
    /// L'instant présent (monotone).
    fn now(&self) -> Instant;
    /// La date présente, format SQLite `YYYY-MM-DD HH:MM:SS` (UTC).
    fn timestamp(&self) -> String;
}

/// L'horloge du système.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn timestamp(&self) -> String {
        // Une horloge système antérieure à 1970 donne l'époque plutôt qu'une erreur
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        domain::sync::timestamp_from_unix(i64::try_from(seconds).unwrap_or(i64::MAX))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_system_clock_writes_valid_timestamps() {
        let clock = SystemClock;
        assert!(domain::sync::is_timestamp(&clock.timestamp()));
        assert!(clock.now() <= Instant::now());
    }
}
