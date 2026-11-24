//! Synchronisation des collections : validation des demandes du renderer (données non
//! fiables) et préparation des événements.

use std::path::{Path, PathBuf};

use app::ports::sync::CollectionOffer;
use domain::profile::ProfileId;
use domain::sync::DeviceKey;
use domain::uuid::Uuid;

use crate::dto::{CollectionOfferDto, LinkedDeviceDto};
use crate::sync::parse_device_key;
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

/// Comment activer une collection reçue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfferMode {
    /// Recevoir dans un dossier vide ou à créer.
    Receive,
    /// Recevoir en fusionnant avec le contenu d'un dossier existant.
    Merge,
}

/// Mode envoyé par le renderer : `receive` ou `merge`.
pub fn parse_offer_mode(text: &str) -> IpcResult<OfferMode> {
    match text {
        "receive" => Ok(OfferMode::Receive),
        "merge" => Ok(OfferMode::Merge),
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

/// Une entrée « à configurer » pour le renderer ; `synced_here` : le profil local déjà
/// synchronisé avec le profil source.
pub fn offer_dto(
    device: &DeviceKey,
    device_name: &str,
    offer: &CollectionOffer,
    synced_here: Option<&ProfileId>,
    received_folder: &str,
) -> CollectionOfferDto {
    CollectionOfferDto {
        device: LinkedDeviceDto {
            key: device.to_string(),
            name: device_name.to_owned(),
        },
        sync_id: offer.share.to_string(),
        name: offer.name.clone(),
        files: offer.files,
        bytes: offer.bytes,
        source_profile_id: offer.source_profile_id.clone(),
        source_profile_name: offer.source_profile_name.clone(),
        source_profile_synced_here: synced_here.map(|profile| profile.as_str().to_owned()),
        suggested_destination: suggested_destination(received_folder, &offer.name),
    }
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
        assert!(parse_offer_mode("decline").is_err());
        assert!(parse_offer_mode("replace").is_err());
        assert_eq!(parse_conflict_keep("mine").unwrap(), ConflictKeep::Mine);
        assert_eq!(parse_conflict_keep("other").unwrap(), ConflictKeep::Other);
        assert!(parse_conflict_keep("both").is_err());
    }

    #[test]
    fn an_entry_to_configure_carries_the_source_profile_and_the_preselected_target() {
        let offer = CollectionOffer {
            share: Uuid::from_random([4; 16]),
            source_profile_id: "work".to_owned(),
            source_profile_name: "Travail".to_owned(),
            source_space: None,
            name: "Docs".to_owned(),
            files: 3,
            bytes: 90,
        };
        let device = DeviceKey::from_bytes([1; 32]);
        let local = ProfileId::parse("copie").unwrap();

        let synced = offer_dto(&device, "Portable", &offer, Some(&local), "/recus");
        let unsynced = offer_dto(&device, "Portable", &offer, None, "/recus");

        assert_eq!(synced.source_profile_synced_here.as_deref(), Some("copie"));
        assert_eq!(unsynced.source_profile_synced_here, None);
        assert_eq!(synced.source_profile_id, "work");
        assert_eq!(synced.source_profile_name, "Travail");
        assert_eq!(synced.device.name, "Portable");
        assert_eq!(synced.sync_id, offer.share.to_string());
        assert!(synced.suggested_destination.ends_with("Docs"));
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
