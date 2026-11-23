//! Identité et appareils de confiance en mémoire.

use std::sync::Mutex;

use domain::sync::{DeviceKey, DeviceKind};
use platform::sync::lock;

use crate::AppResult;
use crate::ports::trust::{DeviceSecret, TrustStore, TrustedDevice};

/// `device.key` et `devices.json` en mémoire.
#[derive(Default)]
pub struct MemoryTrust {
    secret: Mutex<Option<DeviceSecret>>,
    devices: Mutex<Vec<TrustedDevice>>,
}

impl MemoryTrust {
    /// L'entrée d'un appareil, pour vérifier son état.
    pub fn device(&self, key: &DeviceKey) -> Option<TrustedDevice> {
        lock(&self.devices)
            .iter()
            .find(|device| device.key == *key)
            .cloned()
    }
}

impl TrustStore for MemoryTrust {
    fn device_secret(&self) -> AppResult<Option<DeviceSecret>> {
        Ok(lock(&self.secret).clone())
    }

    fn save_device_secret(&self, secret: &DeviceSecret) -> AppResult<()> {
        *lock(&self.secret) = Some(secret.clone());
        Ok(())
    }

    fn devices(&self) -> AppResult<Vec<TrustedDevice>> {
        Ok(lock(&self.devices).clone())
    }

    fn put_device(&self, device: &TrustedDevice) -> AppResult<()> {
        let mut devices = lock(&self.devices);
        devices.retain(|known| known.key != device.key);
        devices.push(device.clone());
        Ok(())
    }

    fn set_device_kind(&self, key: &DeviceKey, kind: DeviceKind) -> AppResult<bool> {
        let mut devices = lock(&self.devices);
        let Some(device) = devices.iter_mut().find(|device| device.key == *key) else {
            return Ok(false);
        };
        let changed = device.kind != kind;
        device.kind = kind;
        Ok(changed)
    }
}
