//! Forme JSON du cache des espaces : `{ "version": 1, "spaces": [{ "spaceId", "profileId",
//! "enabled" }] }`.

use app::AppResult;
use domain::profile::ProfileId;
use domain::sync::SpaceId;
use serde_json::{Value, json};

use super::SpaceIndexEntry;

/// Version du format, pour une évolution future.
const FORMAT_VERSION: u64 = 1;

/// Les entrées en JSON indenté.
pub(super) fn to_text(entries: &[SpaceIndexEntry]) -> AppResult<String> {
    let spaces: Vec<Value> = entries
        .iter()
        .map(|entry| {
            json!({
                "spaceId": entry.space_id.to_string(),
                "profileId": entry.profile.as_str(),
                "enabled": entry.enabled,
            })
        })
        .collect();
    let value = json!({ "version": FORMAT_VERSION, "spaces": spaces });
    serde_json::to_string_pretty(&value).map_err(app::AppError::file_system)
}

/// Les entrées lues, `None` si le JSON n'a pas la forme attendue ; une entrée illisible est
/// ignorée.
pub(super) fn from_text(text: &str) -> Option<Vec<SpaceIndexEntry>> {
    let value: Value = serde_json::from_str(text).ok()?;
    let entries = value.as_object()?.get("spaces")?.as_array()?;
    Some(entries.iter().filter_map(entry_from_json).collect())
}

fn entry_from_json(value: &Value) -> Option<SpaceIndexEntry> {
    let object = value.as_object()?;
    Some(SpaceIndexEntry {
        space_id: SpaceId::parse(object.get("spaceId")?.as_str()?)?,
        profile: ProfileId::parse(object.get("profileId")?.as_str()?)?,
        enabled: object.get("enabled")?.as_bool()?,
    })
}
