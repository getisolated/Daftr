//! Synchronisation des collections : validation des demandes du renderer (données non
//! fiables) et préparation des événements.

use std::path::{Path, PathBuf};

use app::ports::sync::CollectionOffer;
use domain::sync::{DeviceKey, DeviceName};
use domain::uuid::Uuid;

use crate::dto::{CollectionOfferDto, LinkedDeviceDto};
use crate::sync::{linked_device, parse_device_key};
use crate::{IpcError, IpcResult};

/// Appareils au plus dans une demande de partage.
pub const MAX_DEVICES_PER_SHARE: usize = 64;
/// Longueur maximale d'un chemin envoyé par le renderer, en octets.
const MAX_PATH_BYTES: usize = 1024;
/// Nom d'un dossier quand le nom proposé n'en donne aucun utilisable.
const FALLBACK_FOLDER_NAME: &str = "Collection";

/// Identifiant de partage envoyé par le renderer.
pub fn parse_share_id(text: &str) -> IpcResult<Uuid> {
    Uuid::parse(text.trim()).ok_or_else(|| IpcError::invalid_input("invalid share id"))
}

/// Appareils cochés : au moins un, sans doublon, clés valides.
pub fn parse_devices(keys: &[String]) -> IpcResult<Vec<DeviceKey>> {
    if keys.is_empty() || keys.len() > MAX_DEVICES_PER_SHARE {
        return Err(IpcError::invalid_input("invalid device selection"));
    }
    let mut devices: Vec<DeviceKey> = Vec::with_capacity(keys.len());
    for key in keys {
        let device = parse_device_key(key)?;
        if !devices.contains(&device) {
            devices.push(device);
        }
    }
    Ok(devices)
}

/// Dossier de destination choisi : un chemin absolu, sans caractère nul.
pub fn parse_destination(text: &str) -> IpcResult<PathBuf> {
    let path = Path::new(text.trim());
    let valid = !text.trim().is_empty()
        && text.len() <= MAX_PATH_BYTES
        && !text.contains('\0')
        && path.is_absolute();

    if valid {
        Ok(path.to_path_buf())
    } else {
        Err(IpcError::invalid_input("invalid destination folder"))
    }
}

/// Réponse à une collection proposée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OfferMode {
    /// Recevoir dans un dossier vide ou à créer.
    Receive,
    /// Recevoir en fusionnant avec le contenu d'un dossier existant.
    Merge,
    /// Refuser cette proposition.
    Decline,
}

/// Mode envoyé par le renderer : `receive`, `merge` ou `decline`.
pub fn parse_offer_mode(text: &str) -> IpcResult<OfferMode> {
    match text {
        "receive" => Ok(OfferMode::Receive),
        "merge" => Ok(OfferMode::Merge),
        "decline" => Ok(OfferMode::Decline),
        _ => Err(IpcError::invalid_input("invalid offer mode")),
    }
}

/// Version à garder d'une copie de conflit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictKeep {
    /// La version de cet appareil.
    Mine,
    /// La version de l'autre appareil.
    Other,
}

/// Choix envoyé par le renderer : `mine` ou `other`.
pub fn parse_conflict_keep(text: &str) -> IpcResult<ConflictKeep> {
    match text {
        "mine" => Ok(ConflictKeep::Mine),
        "other" => Ok(ConflictKeep::Other),
        _ => Err(IpcError::invalid_input("invalid conflict choice")),
    }
}

/// `<dossier des collections reçues>/<nom>`, le nom rendu utilisable comme nom de dossier.
///
/// ```
/// use presentation::collection_sync::suggested_destination;
///
/// let path = suggested_destination("/home/me/Daftr", "WMS: BC/main");
/// assert!(path.ends_with("WMS_ BC_main"));
/// ```
pub fn suggested_destination(received_folder: &str, name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim_matches(|c: char| c == ' ' || c == '.');
    let folder = if trimmed.is_empty() {
        FALLBACK_FOLDER_NAME
    } else {
        trimmed
    };
    Path::new(received_folder)
        .join(folder)
        .to_string_lossy()
        .into_owned()
}

/// Une proposition pour le renderer.
pub fn offer_dto(
    device: &DeviceKey,
    device_name: &DeviceName,
    profile_id: &str,
    offer: &CollectionOffer,
    received_folder: &str,
) -> CollectionOfferDto {
    CollectionOfferDto {
        device: linked_device(device, device_name),
        profile_id: profile_id.to_owned(),
        sync_id: offer.share.to_string(),
        name: offer.name.clone(),
        files: offer.files,
        bytes: offer.bytes,
        suggested_destination: suggested_destination(received_folder, &offer.name),
    }
}

/// L'appareil d'une proposition, pour le renderer.
pub fn offer_device(device: &DeviceKey, name: &DeviceName) -> LinkedDeviceDto {
    linked_device(device, name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_the_share_and_the_devices() {
        let id = Uuid::from_random([5; 16]);
        assert_eq!(parse_share_id(&id.to_string()).unwrap(), id);
        assert!(parse_share_id("nope").is_err());

        let key = DeviceKey::from_bytes([1; 32]).to_string();
        let devices = parse_devices(&[key.clone(), key]).unwrap();
        assert_eq!(devices.len(), 1);
        assert!(parse_devices(&[]).is_err());
        assert!(parse_devices(&["zz".to_owned()]).is_err());
        let many = vec![DeviceKey::from_bytes([2; 32]).to_string(); MAX_DEVICES_PER_SHARE + 1];
        assert!(parse_devices(&many).is_err());
    }

    #[test]
    fn accepts_an_absolute_destination_only() {
        let absolute = std::env::temp_dir().join("Docs");
        let text = absolute.to_string_lossy();
        assert_eq!(parse_destination(&format!(" {text} ")).unwrap(), absolute);
        assert!(parse_destination("").is_err());
        assert!(parse_destination("relative/docs").is_err());
        assert!(parse_destination("../docs").is_err());
        assert!(parse_destination(&format!("{text}\0")).is_err());
        assert!(parse_destination(&format!("{text}{}", "x".repeat(2000))).is_err());
    }

    #[test]
    fn reads_the_offer_mode_and_the_conflict_choice() {
        assert_eq!(parse_offer_mode("merge").unwrap(), OfferMode::Merge);
        assert_eq!(parse_offer_mode("decline").unwrap(), OfferMode::Decline);
        assert!(parse_offer_mode("replace").is_err());
        assert_eq!(parse_conflict_keep("mine").unwrap(), ConflictKeep::Mine);
        assert_eq!(parse_conflict_keep("other").unwrap(), ConflictKeep::Other);
        assert!(parse_conflict_keep("both").is_err());
    }

    #[test]
    fn names_the_destination_folder_safely() {
        let folder = std::env::temp_dir().to_string_lossy().into_owned();
        let name_of = |name: &str| {
            let path = suggested_destination(&folder, name);
            Path::new(&path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        };
        assert_eq!(name_of("WMS.BusinessCentral"), "WMS.BusinessCentral");
        assert_eq!(name_of("a/b\\c:d"), "a_b_c_d");
        assert_eq!(name_of(" .. "), FALLBACK_FOLDER_NAME);
        assert_eq!(name_of(""), FALLBACK_FOLDER_NAME);
    }
}
