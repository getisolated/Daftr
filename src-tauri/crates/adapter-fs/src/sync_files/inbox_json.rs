//! Forme JSON des propositions de collections en attente : `{ "version": 1, "offers": [{
//! "device", "share", "name", "files", "bytes", "sourceProfileId", "sourceProfileName",
//! "sourceSpace", "ignored" }] }`.

use app::AppResult;
use app::ports::collection_sync::PendingCollection;
use app::ports::sync::CollectionOffer;
use domain::sync::{DeviceKey, SpaceId};
use domain::uuid::Uuid;
use serde_json::{Value, json};

/// Version du format, pour une évolution future.
const FORMAT_VERSION: u64 = 1;

/// Les entrées en JSON indenté.
pub(super) fn to_text(entries: &[PendingCollection]) -> AppResult<String> {
    let offers: Vec<Value> = entries
        .iter()
        .map(|entry| {
            json!({
                "device": entry.device.to_string(),
                "share": entry.offer.share.to_string(),
                "name": entry.offer.name,
                "files": entry.offer.files,
                "bytes": entry.offer.bytes,
                "sourceProfileId": entry.offer.source_profile_id,
                "sourceProfileName": entry.offer.source_profile_name,
                "sourceSpace": entry.offer.source_space.map(|space| space.to_string()),
                "ignored": entry.ignored,
            })
        })
        .collect();
    let value = json!({ "version": FORMAT_VERSION, "offers": offers });
    serde_json::to_string_pretty(&value).map_err(app::AppError::file_system)
}

/// Les entrées lues, `None` si le JSON n'a pas la forme attendue ; une entrée illisible est
/// ignorée (le proposant la renverra).
pub(super) fn from_text(text: &str) -> Option<Vec<PendingCollection>> {
    let value: Value = serde_json::from_str(text).ok()?;
    let offers = value.as_object()?.get("offers")?.as_array()?;
    Some(offers.iter().filter_map(entry_from_json).collect())
}

fn entry_from_json(value: &Value) -> Option<PendingCollection> {
    let object = value.as_object()?;
    let text = |key: &str| object.get(key)?.as_str().map(str::to_owned);
    let number = |key: &str| object.get(key)?.as_u64();
    Some(PendingCollection {
        device: DeviceKey::parse(&text("device")?)?,
        offer: CollectionOffer {
            share: Uuid::parse(&text("share")?)?,
            source_profile_id: text("sourceProfileId").unwrap_or_default(),
            source_profile_name: text("sourceProfileName").unwrap_or_default(),
            source_space: text("sourceSpace").and_then(|space| SpaceId::parse(&space)),
            name: text("name")?,
            files: number("files")?,
            bytes: number("bytes")?,
        },
        ignored: object.get("ignored")?.as_bool()?,
    })
}
