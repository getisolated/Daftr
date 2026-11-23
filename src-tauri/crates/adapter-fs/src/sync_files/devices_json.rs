//! Forme JSON des appareils de confiance : `{ "version": 1, "devices": [{ "key", "name",
//! "origin": "paired" | "space", "addedAt", "revokedAt" }] }`.

use app::AppResult;
use app::ports::trust::{TrustOrigin, TrustedDevice};
use domain::sync::{DeviceKey, DeviceName};
use serde_json::{Map, Value, json};

/// Version du format, pour une évolution future.
const FORMAT_VERSION: u64 = 1;
const PAIRED: &str = "paired";
const SPACE: &str = "space";

/// Les appareils en JSON indenté.
pub(super) fn to_text(devices: &[TrustedDevice]) -> AppResult<String> {
    let devices: Vec<Value> = devices.iter().map(device_to_json).collect();
    let value = json!({ "version": FORMAT_VERSION, "devices": devices });
    serde_json::to_string_pretty(&value).map_err(app::AppError::file_system)
}

fn device_to_json(device: &TrustedDevice) -> Value {
    json!({
        "key": device.key.to_string(),
        "name": device.name.as_str(),
        "origin": match device.origin {
            TrustOrigin::Paired => PAIRED,
            TrustOrigin::Space => SPACE,
        },
        "addedAt": device.added_at,
        "revokedAt": device.revoked_at,
    })
}

/// Les appareils lus, `None` si le JSON n'a pas la forme attendue. Une entrée illisible est
/// ignorée (journalisée).
pub(super) fn from_text(text: &str) -> Option<Vec<TrustedDevice>> {
    let value: Value = serde_json::from_str(text).ok()?;
    let entries = value.as_object()?.get("devices")?.as_array()?;
    Some(entries.iter().filter_map(device_from_json).collect())
}

/// Une révocation n'est jamais perdue : toute valeur textuelle de `revokedAt` la garde.
fn device_from_json(value: &Value) -> Option<TrustedDevice> {
    let object = value.as_object()?;
    let key = text(object, "key").and_then(DeviceKey::parse);
    let name = text(object, "name").and_then(DeviceName::sanitize);
    let (Some(key), Some(name)) = (key, name) else {
        tracing::warn!("appareil de confiance illisible ignoré");
        return None;
    };
    let origin = match text(object, "origin") {
        Some(SPACE) => TrustOrigin::Space,
        _ => TrustOrigin::Paired,
    };
    Some(TrustedDevice {
        key,
        name,
        origin,
        added_at: text(object, "addedAt").unwrap_or_default().to_owned(),
        revoked_at: text(object, "revokedAt").map(str::to_owned),
    })
}

fn text<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    object.get(key).and_then(Value::as_str)
}
