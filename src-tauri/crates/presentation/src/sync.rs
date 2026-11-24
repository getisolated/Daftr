//! Synchronisation entre appareils : validation des demandes du renderer (données non
//! fiables) et événements préparés pour l'interface. Le moteur (`adapter-sync`) est branché
//! par le crate `desktop`.

use std::net::{IpAddr, SocketAddr};

use app::ports::sync::{CollectionOffer, SyncEvent};
use domain::profile::ProfileId;
use domain::settings::{DEFAULT_SYNC_PORT, SYNC_MAX_PORT, SYNC_MIN_PORT};
use domain::sync::{DeviceKey, DeviceName, PairingCode};

use crate::dto::{
    CollectionProgressDto, InitialSyncDoneDto, LinkedDeviceDto, NotesSyncedDto, ProfileReceivedDto,
    SyncProgressDto, SyncStatusDto,
};
use crate::{IpcError, IpcResult};

/// Longueur maximale d'une adresse saisie.
const MAX_ADDRESS_LENGTH: usize = 64;

/// Clé d'appareil envoyée par le renderer (64 chiffres hexadécimaux).
pub fn parse_device_key(text: &str) -> IpcResult<DeviceKey> {
    DeviceKey::parse(text.trim()).ok_or_else(|| IpcError::invalid_input("invalid device key"))
}

/// Code d'appairage saisi : six chiffres, espaces ignorés.
pub fn parse_pairing_code(text: &str) -> IpcResult<PairingCode> {
    PairingCode::parse(text).ok_or_else(|| IpcError::invalid_input("invalid pairing code"))
}

/// Adresse saisie en repli (`IP:port`, `[IPv6]:port`, ou une IP seule sur le port par
/// défaut) ; vide : `None` (découverte sur le réseau). Le moteur refuse ensuite toute
/// adresse hors du réseau local.
///
/// ```
/// use presentation::sync::parse_address;
///
/// let address = parse_address(Some(" 192.168.1.20:48000 ")).unwrap().unwrap();
/// assert_eq!(address.to_string(), "192.168.1.20:48000");
/// assert_eq!(parse_address(Some("10.0.0.2")).unwrap().unwrap().port(), 47820);
/// assert_eq!(parse_address(Some("")).unwrap(), None);
/// assert!(parse_address(Some("pc.local:80")).is_err());
/// ```
pub fn parse_address(text: Option<&str>) -> IpcResult<Option<SocketAddr>> {
    let text = text.map(str::trim).unwrap_or_default();
    if text.is_empty() {
        return Ok(None);
    }
    let invalid = || IpcError::invalid_input("invalid address");
    if text.len() > MAX_ADDRESS_LENGTH {
        return Err(invalid());
    }
    if let Ok(address) = text.parse::<SocketAddr>() {
        return Ok(Some(address));
    }
    let bare = text.trim_start_matches('[').trim_end_matches(']');
    let ip: IpAddr = bare.parse().map_err(|_| invalid())?;
    Ok(Some(SocketAddr::new(ip, DEFAULT_SYNC_PORT)))
}

/// Nom de l'appareil saisi : vide pour revenir au nom d'hôte, sinon un nom valide.
pub fn parse_device_name(text: &str) -> IpcResult<String> {
    if text.trim().is_empty() {
        return Ok(String::new());
    }
    DeviceName::parse(text)
        .map(|name| name.as_str().to_owned())
        .ok_or_else(|| IpcError::invalid_input("invalid device name"))
}

/// Port saisi, hors des ports réservés au système.
pub fn parse_port(port: u32) -> IpcResult<u16> {
    u16::try_from(port)
        .ok()
        .filter(|port| (SYNC_MIN_PORT..=SYNC_MAX_PORT).contains(port))
        .ok_or_else(|| IpcError::invalid_input("invalid port"))
}

/// Un événement de la synchronisation prêt pour le renderer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncEventDto {
    /// `sync:notes-synced`.
    NotesSynced(NotesSyncedDto),
    /// `sync:status`.
    Status(SyncStatusDto),
    /// `sync:progress`.
    Progress(SyncProgressDto),
    /// `sync:initial-done`.
    InitialSyncDone(InitialSyncDoneDto),
    /// `sync:device-linked`.
    DeviceLinked(LinkedDeviceDto),
    /// `sync:profile-received`.
    ProfileReceived(ProfileReceivedDto),
    /// Nouvelles collections reçues « à configurer » : le crate `desktop` complète chaque
    /// proposition (profil source déjà synchronisé ici, destination proposée) et émet
    /// `sync:collection-offer`.
    CollectionOffered {
        /// L'appareil qui propose.
        device: DeviceKey,
        /// Les nouvelles propositions.
        offers: Vec<CollectionOffer>,
    },
    /// Des entrées « à configurer » ont disparu : le crate `desktop` relit la liste et émet
    /// `sync:collection-offer`.
    CollectionPendingChanged,
    /// `sync:collection-progress`.
    CollectionProgress(CollectionProgressDto),
    /// L'état d'un partage a changé : le crate `desktop` le relit et émet
    /// `sync:collection-state` (et `sync:collection-conflicts` si les copies de conflit ont
    /// changé).
    CollectionChanged {
        /// Profil local.
        profile_id: String,
        /// Partage.
        sync_id: String,
    },
}

impl SyncEventDto {
    /// L'événement du moteur ; `active` : le profil actif, pour marquer ses notes.
    pub fn from_event(event: &SyncEvent<ProfileId>, active: &ProfileId) -> Self {
        match event {
            SyncEvent::NotesSynced {
                profile,
                notes,
                folders,
                from,
            } => Self::NotesSynced(NotesSyncedDto {
                profile_id: profile.as_str().to_owned(),
                active: profile == active,
                uuids: notes.iter().map(ToString::to_string).collect(),
                folders: *folders,
                device_name: from.as_ref().map(|name| name.as_str().to_owned()),
            }),
            SyncEvent::Status(status) => Self::Status(status.into()),
            SyncEvent::Progress {
                profile,
                done,
                total,
            } => Self::Progress(SyncProgressDto {
                profile_id: profile.as_str().to_owned(),
                done: *done,
                total: *total,
            }),
            SyncEvent::InitialSyncDone { profile } => {
                Self::InitialSyncDone(InitialSyncDoneDto::new(profile))
            }
            SyncEvent::DeviceLinked { device, name } => {
                Self::DeviceLinked(linked_device(device, name))
            }
            SyncEvent::ProfileReceived {
                profile,
                name,
                device,
                device_name,
            } => Self::ProfileReceived(ProfileReceivedDto {
                profile_id: profile.as_str().to_owned(),
                name: name.clone(),
                device: linked_device(device, device_name),
            }),
            SyncEvent::CollectionOfferReceived {
                device,
                name: _,
                offers,
            } => Self::CollectionOffered {
                device: *device,
                offers: offers.clone(),
            },
            SyncEvent::CollectionPendingChanged => Self::CollectionPendingChanged,
            SyncEvent::CollectionProgress {
                profile,
                share,
                device,
                done,
                total,
            } => Self::CollectionProgress(CollectionProgressDto {
                profile_id: profile.as_str().to_owned(),
                sync_id: share.to_string(),
                device_key: device.to_string(),
                done: *done,
                total: *total,
            }),
            SyncEvent::CollectionChanged { profile, share } => Self::CollectionChanged {
                profile_id: profile.as_str().to_owned(),
                sync_id: share.to_string(),
            },
        }
    }
}

/// Un appareil lié, pour le renderer.
pub fn linked_device(key: &DeviceKey, name: &DeviceName) -> LinkedDeviceDto {
    LinkedDeviceDto {
        key: key.to_string(),
        name: name.as_str().to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use app::ports::sync::SyncStatus;
    use domain::uuid::Uuid;

    use super::*;
    use crate::dto::SyncStatusKind;

    #[test]
    fn validates_keys_and_codes() {
        let key = DeviceKey::from_bytes([1; 32]);
        assert_eq!(parse_device_key(&key.to_string()).unwrap(), key);
        assert!(parse_device_key("abc").is_err());
        assert_eq!(parse_pairing_code("123 456").unwrap().as_str(), "123456");
        assert!(parse_pairing_code("12a456").is_err());
    }

    #[test]
    fn reads_the_fallback_address() {
        let v6 = parse_address(Some("[fe80::1]:48000")).unwrap().unwrap();
        assert_eq!(v6.port(), 48000);
        let bare_v6 = parse_address(Some("[fd00::2]")).unwrap().unwrap();
        assert_eq!(bare_v6.port(), DEFAULT_SYNC_PORT);
        assert_eq!(parse_address(None).unwrap(), None);
        assert!(parse_address(Some("1.2.3.4:99999")).is_err());
        assert!(parse_address(Some(&"1".repeat(100))).is_err());
    }

    #[test]
    fn validates_the_device_settings() {
        assert_eq!(parse_device_name("  Laptop ").unwrap(), "Laptop");
        assert_eq!(parse_device_name("  ").unwrap(), "");
        assert!(parse_device_name("PC\u{0}").is_err());
        assert_eq!(parse_port(48000).unwrap(), 48000);
        assert!(parse_port(80).is_err());
        assert!(parse_port(70000).is_err());
    }

    #[test]
    fn prepares_the_events_for_the_renderer() {
        let work = ProfileId::parse("work").unwrap();
        let uuid = Uuid::from_random([7; 16]);
        let synced = SyncEvent::NotesSynced {
            profile: work.clone(),
            notes: vec![uuid],
            folders: true,
            from: Some(DeviceName::parse("Laptop").unwrap()),
        };
        let SyncEventDto::NotesSynced(dto) = SyncEventDto::from_event(&synced, &work) else {
            panic!("événement inattendu");
        };
        assert!(dto.active && dto.folders);
        assert_eq!(dto.uuids, [uuid.to_string()]);
        assert_eq!(dto.device_name.as_deref(), Some("Laptop"));

        let status = SyncEvent::Status(SyncStatus::Error {
            reason: "listen_failed".to_owned(),
        });
        let other = ProfileId::default_profile();
        let SyncEventDto::Status(dto) = SyncEventDto::from_event(&status, &other) else {
            panic!("événement inattendu");
        };
        assert_eq!(dto.state, SyncStatusKind::Error);
        assert_eq!(dto.reason.as_deref(), Some("listen_failed"));

        let received = SyncEvent::ProfileReceived {
            profile: work.clone(),
            name: "Travail".to_owned(),
            device: DeviceKey::from_bytes([2; 32]),
            device_name: DeviceName::parse("Laptop").unwrap(),
        };
        let SyncEventDto::ProfileReceived(dto) = SyncEventDto::from_event(&received, &other) else {
            panic!("événement inattendu");
        };
        assert_eq!(
            (dto.profile_id.as_str(), dto.name.as_str()),
            ("work", "Travail")
        );
        assert_eq!(dto.device.name, "Laptop");
    }
}
