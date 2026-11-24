//! Appareils membres de l'espace. Un appareil est accepté pour l'espace s'il y figure sans
//! être révoqué ; la confiance est donc transitive au sein de l'espace.

use domain::sync::{DeviceKey, DeviceName};
use yrs::{Any, In, Map, MapPrelim, MapRef, Out, ReadTxn, Transact};

use super::SpaceDoc;
use crate::sync::values;

pub(super) const NAME: &str = "name";
pub(super) const ADDED_AT: &str = "added_at";
pub(super) const REVOKED_AT: &str = "revoked_at";

/// Ajoute l'appareil à l'espace à la date `now`, ou le réadmet s'il avait été révoqué (après
/// un nouvel appairage ou une offre acceptée). Rend `true` si le document a changé.
pub fn add_device(space: &SpaceDoc, key: &DeviceKey, name: &DeviceName, now: &str) -> bool {
    let mut transaction = space.doc.transact_mut();
    if let Some(device) = entry(space, &transaction, key) {
        let renamed = values::set(&device, &mut transaction, NAME, name.as_str().into());
        return values::set(&device, &mut transaction, REVOKED_AT, Any::Null) | renamed;
    }

    let entries: [(&str, Any); 3] = [
        (NAME, name.as_str().into()),
        (ADDED_AT, now.into()),
        (REVOKED_AT, Any::Null),
    ];
    let prelim: MapPrelim = entries
        .into_iter()
        .map(|(key, value)| (key, In::from(value)))
        .collect();
    space
        .devices
        .insert(&mut transaction, key.to_string(), prelim);
    true
}

/// Renomme un appareil connu, sans changer sa révocation. Rend `true` si le document a changé.
pub fn rename_device(space: &SpaceDoc, key: &DeviceKey, name: &DeviceName) -> bool {
    let mut transaction = space.doc.transact_mut();
    let Some(device) = entry(space, &transaction, key) else {
        return false;
    };
    values::set(&device, &mut transaction, NAME, name.as_str().into())
}

/// Révoque l'appareil à la date `now` : il ne peut plus échanger cet espace. Rend `true` s'il
/// était membre.
pub fn revoke_device(space: &SpaceDoc, key: &DeviceKey, now: &str) -> bool {
    let mut transaction = space.doc.transact_mut();
    let Some(device) = entry(space, &transaction, key) else {
        return false;
    };
    if values::text(&device, &transaction, REVOKED_AT).is_some() {
        return false;
    }
    values::set(&device, &mut transaction, REVOKED_AT, now.into())
}

fn entry<T: ReadTxn>(space: &SpaceDoc, transaction: &T, key: &DeviceKey) -> Option<MapRef> {
    match space.devices.get(transaction, &key.to_string())? {
        Out::YMap(device) => Some(device),
        _ => None,
    }
}
