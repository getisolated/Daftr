//! Synchronisation entre appareils : état, appareils de confiance, appairage, événements.
//!
//! Les clés d'appareil et les espaces sont en hexadécimal (64 chiffres) ; les profils par
//! leur identifiant local.

use app::ports::sync::{ProfileOffer, SyncStatus};
use app::ports::trust::{TrustOrigin, TrustedDevice};
use domain::profile::ProfileId;
use serde::Serialize;
use ts_rs::TS;

/// État de la barre d'état.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SyncStatusKind {
    /// Aucun profil synchronisé.
    Disabled,
    /// Pause globale.
    Paused,
    /// À jour avec `devices` appareils connectés.
    UpToDate,
    /// Échange en cours.
    Syncing,
    /// Aucun appareil de confiance sur ce réseau.
    NoDevice,
    /// Échec, cause dans `reason`.
    Error,
}

/// État de la synchronisation (événement `sync:status`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SyncStatusDto {
    /// État.
    pub state: SyncStatusKind,
    /// Appareils connectés (`upToDate`).
    pub devices: Option<u32>,
    /// Cause stable de l'erreur (`listen_failed`, `discovery_failed`…), traduite par le
    /// renderer.
    pub reason: Option<String>,
}

impl From<&SyncStatus> for SyncStatusDto {
    fn from(status: &SyncStatus) -> Self {
        let (state, devices, reason) = match status {
            SyncStatus::Disabled => (SyncStatusKind::Disabled, None, None),
            SyncStatus::Paused => (SyncStatusKind::Paused, None, None),
            SyncStatus::UpToDate { devices } => {
                let count = u32::try_from(*devices).unwrap_or(u32::MAX);
                (SyncStatusKind::UpToDate, Some(count), None)
            }
            SyncStatus::Syncing => (SyncStatusKind::Syncing, None, None),
            SyncStatus::NoDevice => (SyncStatusKind::NoDevice, None, None),
            SyncStatus::Error { reason } => (SyncStatusKind::Error, None, Some(reason.clone())),
        };
        Self {
            state,
            devices,
            reason,
        }
    }
}

/// Un profil synchronisé de cet appareil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SyncedProfileDto {
    /// Profil local.
    pub profile_id: String,
    /// Son espace.
    pub space_id: String,
    /// Sa synchronisation est activée.
    pub enabled: bool,
}

/// Vue d'ensemble pour Réglages → Synchronisation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SyncStateDto {
    /// État de la barre d'état.
    pub status: SyncStatusDto,
    /// Clé publique de cet appareil.
    pub device_key: String,
    /// Nom effectif de cet appareil (réglage ou nom d'hôte).
    pub device_name: String,
    /// Le nom est celui de l'hôte (réglage vide).
    pub device_name_is_default: bool,
    /// Port configuré.
    pub port: u16,
    /// Port d'écoute effectif, `null` quand rien n'écoute.
    pub listening_port: Option<u16>,
    /// Pause globale.
    pub paused: bool,
    /// Profils synchronisés, actifs ou non.
    pub profiles: Vec<SyncedProfileDto>,
}

/// Origine de la confiance envers un appareil.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TrustOriginDto {
    /// Appairé directement.
    Paired,
    /// Connu via un profil commun.
    Space,
}

impl From<TrustOrigin> for TrustOriginDto {
    fn from(origin: TrustOrigin) -> Self {
        match origin {
            TrustOrigin::Paired => Self::Paired,
            TrustOrigin::Space => Self::Space,
        }
    }
}

/// Un profil partagé avec un appareil de confiance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SharedProfileDto {
    /// Profil local.
    pub profile_id: String,
    /// L'appareil a été retiré de ce profil (« Arrêter avec… »).
    pub revoked: bool,
    /// Dernier échange de ce profil avec l'appareil (UTC, format SQLite).
    pub last_sync_at: Option<String>,
}

/// Un appareil de confiance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TrustedDeviceDto {
    /// Clé publique.
    pub key: String,
    /// Nom.
    pub name: String,
    /// Comment il est devenu de confiance.
    pub origin: TrustOriginDto,
    /// Date d'ajout (UTC).
    pub added_at: String,
    /// Date de retrait : retiré pour de bon, seul un nouvel appairage le réadmet.
    pub revoked_at: Option<String>,
    /// Visible sur le réseau ou connecté.
    pub present: bool,
    /// Connecté.
    pub connected: bool,
    /// Profils partagés avec lui.
    pub profiles: Vec<SharedProfileDto>,
}

impl TrustedDeviceDto {
    /// L'appareil, avec sa présence et ses profils partagés.
    pub fn new(
        device: &TrustedDevice,
        presence: (bool, bool),
        profiles: Vec<SharedProfileDto>,
    ) -> Self {
        let (present, connected) = presence;
        Self {
            key: device.key.to_string(),
            name: device.name.as_str().to_owned(),
            origin: device.origin.into(),
            added_at: device.added_at.clone(),
            revoked_at: device.revoked_at.clone(),
            present: present || connected,
            connected,
            profiles,
        }
    }
}

/// Un membre de l'espace d'un profil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SpaceDeviceDto {
    /// Clé publique.
    pub key: String,
    /// Nom.
    pub name: String,
    /// C'est cet appareil.
    pub is_self: bool,
    /// Retiré de l'espace.
    pub revoked: bool,
}

/// La synchronisation d'un profil (réglages du profil).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProfileSyncDto {
    /// Profil local.
    pub profile_id: String,
    /// Son espace, `null` s'il n'est pas synchronisé.
    pub space_id: Option<String>,
    /// Sa synchronisation est activée.
    pub enabled: bool,
    /// Membres de l'espace (cet appareil compris).
    pub devices: Vec<SpaceDeviceDto>,
}

/// Mode appairage ouvert : ce qu'affiche « Lier un appareil ».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PairingTicketDto {
    /// Code à 6 chiffres.
    pub code: String,
    /// Contenu du QR code (`daftr-pair:1:…`).
    pub uri: String,
    /// QR code en SVG.
    pub qr_svg: String,
    /// Fin du mode, en millisecondes depuis l'époque Unix.
    #[ts(type = "number")]
    pub expires_at: u64,
}

/// Suivi du mode appairage (événement `sync:pairing`) : nouveau code après trop d'échecs,
/// ou fin du mode (`ticket` absent).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PairingStateDto {
    /// Le mode est ouvert.
    pub active: bool,
    /// Code et QR code courants.
    pub ticket: Option<PairingTicketDto>,
}

/// Un appareil (appairage terminé : événement `sync:device-linked` et réponse de
/// `sync_pair`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LinkedDeviceDto {
    /// Clé publique.
    pub key: String,
    /// Nom.
    pub name: String,
}

/// Un profil proposé par un appareil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProfileOfferDto {
    /// Espace proposé (à renvoyer dans la réponse).
    pub space_id: String,
    /// Nom proposé.
    pub name: String,
    /// Nombre de notes.
    #[ts(type = "number")]
    pub note_count: u64,
}

impl From<&ProfileOffer> for ProfileOfferDto {
    fn from(offer: &ProfileOffer) -> Self {
        Self {
            space_id: offer.space_id.to_string(),
            name: offer.name.clone(),
            note_count: offer.note_count,
        }
    }
}

/// Offre de profils reçue (événement `sync:offer-received`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OfferReceivedDto {
    /// L'appareil qui propose.
    pub device: LinkedDeviceDto,
    /// Les profils proposés : répondre à chacun par `sync_answer_offer`.
    pub offers: Vec<ProfileOfferDto>,
}

/// Notes modifiées par un autre appareil (événement `sync:notes-synced`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NotesSyncedDto {
    /// Profil local.
    pub profile_id: String,
    /// C'est le profil actif : listes, sidebar et note ouverte sont à relire.
    pub active: bool,
    /// Uuid des notes modifiées.
    pub uuids: Vec<String>,
    /// Les dossiers ont changé.
    pub folders: bool,
    /// Nom de l'appareil d'où viennent les modifications (« modifié depuis… »), s'il est connu.
    pub device_name: Option<String>,
}

/// Avancement de la synchronisation initiale (événement `sync:progress`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SyncProgressDto {
    /// Profil local.
    pub profile_id: String,
    /// Documents traités.
    #[ts(type = "number")]
    pub done: u64,
    /// Documents à traiter.
    #[ts(type = "number")]
    pub total: u64,
}

/// Synchronisation initiale d'un profil terminée (événement `sync:initial-done`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InitialSyncDoneDto {
    /// Profil local.
    pub profile_id: String,
}

impl InitialSyncDoneDto {
    /// Pour `profile`.
    pub fn new(profile: &ProfileId) -> Self {
        Self {
            profile_id: profile.as_str().to_owned(),
        }
    }
}
