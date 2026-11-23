//! Port de l'identité de l'appareil et des appareils de confiance (`<data root>/sync/` :
//! `device.key`, `devices.json`), communs à tous les profils.

use std::fmt;

use domain::sync::{DeviceKey, DeviceKind, DeviceName};

use crate::AppResult;

/// Clé privée statique x25519 de l'appareil (`device.key`). Elle n'apparaît jamais dans les
/// traces ; la clé publique ([`DeviceKey`]) en est dérivée par le transport.
#[derive(Clone, PartialEq, Eq)]
pub struct DeviceSecret([u8; 32]);

impl DeviceSecret {
    /// Les 32 octets tels quels.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Les 32 octets.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// La clé privée n'apparaît jamais dans les traces.
impl fmt::Debug for DeviceSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DeviceSecret(******)")
    }
}

/// Comment un appareil est devenu de confiance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustOrigin {
    /// Appairé directement (code ou QR code).
    Paired,
    /// Connu via un profil commun : membre d'un espace partagé avec cet appareil.
    Space,
}

/// Un appareil de confiance (une entrée de `devices.json`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustedDevice {
    /// Sa clé publique, son identifiant.
    pub key: DeviceKey,
    /// Son nom.
    pub name: DeviceName,
    /// Son type, tel qu'il l'a annoncé à la dernière session.
    pub kind: DeviceKind,
    /// Comment il est devenu de confiance.
    pub origin: TrustOrigin,
    /// Date d'ajout (format SQLite, UTC).
    pub added_at: String,
    /// Date de révocation : définitive, seul un nouvel appairage le réadmet.
    pub revoked_at: Option<String>,
}

impl TrustedDevice {
    /// L'appareil est de confiance (non révoqué).
    pub fn is_trusted(&self) -> bool {
        self.revoked_at.is_none()
    }
}

/// Identité de l'appareil et appareils de confiance, écrits de façon atomique.
pub trait TrustStore: Send + Sync {
    /// La clé privée de l'appareil, `None` avant sa création.
    fn device_secret(&self) -> AppResult<Option<DeviceSecret>>;
    /// Enregistre la clé privée de l'appareil (permissions restreintes).
    fn save_device_secret(&self, secret: &DeviceSecret) -> AppResult<()>;
    /// Les appareils connus, révoqués compris.
    fn devices(&self) -> AppResult<Vec<TrustedDevice>>;
    /// Ajoute l'appareil ou remplace l'entrée de même clé.
    fn put_device(&self, device: &TrustedDevice) -> AppResult<()>;
    /// Change le type d'un appareil connu, en une lecture-écriture indivisible (une
    /// révocation écrite entre-temps n'est jamais perdue). Sans effet pour un appareil
    /// inconnu ; rend `true` si l'entrée a changé.
    fn set_device_kind(&self, key: &DeviceKey, kind: DeviceKind) -> AppResult<bool>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_secret_stays_out_of_debug_output() {
        let secret = DeviceSecret::from_bytes([0xab; 32]);
        assert!(!format!("{secret:?}").contains("ab"));
        assert_eq!(secret.as_bytes(), &[0xab; 32]);
    }
}
