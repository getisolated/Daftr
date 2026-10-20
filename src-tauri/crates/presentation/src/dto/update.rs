//! Mises à jour.

use serde::Serialize;
use ts_rs::TS;

/// Mise à jour trouvée.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateInfoDto {
    /// Version installée.
    pub current_version: String,
    /// Version publiée.
    pub version: String,
    /// L'installeur est téléchargé et vérifié : l'installation est immédiate.
    pub downloaded: bool,
}

/// Avancement du téléchargement ; `percent` vaut -1 quand la taille totale est inconnue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct UpdateProgressDto {
    /// Octets reçus.
    #[ts(type = "number")]
    pub received: u64,
    /// Taille totale (0 si inconnue).
    #[ts(type = "number")]
    pub total: u64,
    /// Pourcentage entier, -1 si inconnu.
    pub percent: i32,
}

impl UpdateProgressDto {
    /// Avancement d'un téléchargement.
    pub fn new(received: u64, total: Option<u64>) -> Self {
        let total = total.unwrap_or(0);
        let percent = match total {
            0 => -1,
            _ => i32::try_from(received.saturating_mul(100) / total)
                .unwrap_or(100)
                .min(100),
        };
        Self {
            received,
            total,
            percent,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_the_percentage_when_the_size_is_known() {
        assert_eq!(UpdateProgressDto::new(50, Some(200)).percent, 25);
        assert_eq!(UpdateProgressDto::new(10, None).percent, -1);
        assert_eq!(UpdateProgressDto::new(300, Some(200)).percent, 100);
    }
}
