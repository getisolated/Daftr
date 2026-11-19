//! Nouvelles tentatives après un échec de connexion : délai doublé à chaque échec, borné.

use std::time::Duration;

/// Bornes du délai entre deux tentatives vers la dernière adresse connue d'un appareil.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackoffPolicy {
    /// Délai après le premier échec.
    pub min: Duration,
    /// Délai maximal.
    pub max: Duration,
}

impl Default for BackoffPolicy {
    /// De 30 s à 10 min.
    fn default() -> Self {
        Self {
            min: Duration::from_secs(30),
            max: Duration::from_secs(10 * 60),
        }
    }
}

impl BackoffPolicy {
    /// Délai avant la tentative qui suit `failures` échecs consécutifs (au moins un).
    pub fn delay(&self, failures: u32) -> Duration {
        let doublings = failures.saturating_sub(1).min(31);
        self.min
            .checked_mul(1_u32 << doublings)
            .map_or(self.max, |delay| delay.min(self.max))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_delay_doubles_from_30_seconds_to_10_minutes() {
        let policy = BackoffPolicy::default();
        let delays: Vec<u64> = (1..=7).map(|n| policy.delay(n).as_secs()).collect();
        assert_eq!(delays, [30, 60, 120, 240, 480, 600, 600]);
        assert_eq!(policy.delay(0).as_secs(), 30);
        assert_eq!(policy.delay(u32::MAX).as_secs(), 600);
    }
}
