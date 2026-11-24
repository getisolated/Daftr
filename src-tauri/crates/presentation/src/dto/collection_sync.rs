//! Synchronisation des collections entre appareils : partages, propositions, conflits.
//!
//! Les identifiants de partage sont des uuid ; les appareils, leur clé en hexadécimal ; les
//! chemins sont ceux de l'appareil (jamais échangés).

use serde::Serialize;
use ts_rs::TS;

use super::LinkedDeviceDto;

/// État d'un partage, dans l'ordre où il s'impose à l'affichage (pastille de la sidebar).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CollectionShareStatusKind {
    /// Le dossier n'existe plus : « Relier à un autre dossier ».
    MissingFolder,
    /// Un échange de fichiers est en cours (`done` / `total`).
    Syncing,
    /// Des copies de conflit attendent un choix.
    Conflicts,
    /// Aucun des appareils du partage n'est connecté.
    Waiting,
    /// Proposée, aucun appareil n'a encore accepté.
    Offered,
    /// À jour.
    UpToDate,
}

/// Où en est un appareil pour un partage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CollectionPeerStateDto {
    /// La collection lui a été proposée.
    Offered,
    /// Il l'a acceptée.
    Active,
}

/// Pourquoi un fichier n'est pas synchronisé.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum IgnoredReasonDto {
    /// Plus de 10 Mio.
    TooLarge,
    /// Un autre fichier ne diffère de lui que par la casse.
    CaseCollision,
    /// Nom que les autres systèmes ne peuvent pas porter.
    InvalidPath,
    /// Illisible.
    Unreadable,
}

/// Un fichier écarté.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct IgnoredFileDto {
    /// Chemin relatif à la collection.
    pub path: String,
    /// Raison.
    pub reason: IgnoredReasonDto,
}

/// Un appareil d'un partage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CollectionShareDeviceDto {
    /// Clé de l'appareil.
    pub key: String,
    /// Son nom (celui de l'appareil de confiance).
    pub name: String,
    /// Proposé ou actif.
    pub state: CollectionPeerStateDto,
    /// Une session est ouverte avec lui.
    pub connected: bool,
    /// Un échange de fichiers est en cours avec lui.
    pub syncing: bool,
    /// Dernier échange complet (format SQLite, UTC).
    pub last_sync_at: Option<String>,
}

/// Une copie de conflit `nom (conflit <appareil>).md` à côté de son original.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CollectionConflictDto {
    /// Chemin relatif de la copie.
    pub path: String,
    /// Chemin relatif du fichier d'origine.
    pub original: String,
    /// Nom de l'appareil dont la copie garde la version.
    pub device_name: String,
    /// La copie garde la version de cet appareil-ci.
    pub mine: bool,
}

/// Une collection partagée.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CollectionShareDto {
    /// Profil qui porte la collection.
    pub profile_id: String,
    /// Identifiant du partage (commun aux appareils).
    pub sync_id: String,
    /// Chemin de la collection sur cet appareil (sa clé dans `collections_list`).
    pub root: String,
    /// Nom affiché.
    pub name: String,
    /// État affiché par la pastille.
    pub status: CollectionShareStatusKind,
    /// Fichiers synchronisés.
    #[ts(type = "number")]
    pub files: u64,
    /// Fichiers traités par l'échange en cours.
    #[ts(type = "number")]
    pub done: u64,
    /// Fichiers à traiter par l'échange en cours.
    #[ts(type = "number")]
    pub total: u64,
    /// Les appareils du partage.
    pub devices: Vec<CollectionShareDeviceDto>,
    /// Les copies de conflit présentes.
    pub conflicts: Vec<CollectionConflictDto>,
    /// Les fichiers écartés.
    pub ignored: Vec<IgnoredFileDto>,
    /// Le parcours s'est arrêté à la limite de fichiers ou de profondeur.
    pub truncated: bool,
}

/// Collections partagées et propositions en attente (commande `collection_sync_state`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CollectionSyncStateDto {
    /// Les collections partagées de tous les profils synchronisés.
    pub shares: Vec<CollectionShareDto>,
    /// Les propositions qui attendent une réponse.
    pub offers: Vec<CollectionOfferDto>,
    /// Dossier où arrivent les collections reçues (le réglage, ou `Documents/Daftr`).
    pub received_folder: String,
}

/// Une collection proposée par un appareil.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CollectionOfferDto {
    /// L'appareil qui propose.
    pub device: LinkedDeviceDto,
    /// Profil qui recevra la collection.
    pub profile_id: String,
    /// Identifiant du partage, à renvoyer dans la réponse.
    pub sync_id: String,
    /// Nom proposé.
    pub name: String,
    /// Nombre de fichiers `.md`.
    #[ts(type = "number")]
    pub files: u64,
    /// Volume des fichiers, en octets.
    #[ts(type = "number")]
    pub bytes: u64,
    /// Destination proposée : `<dossier des collections reçues>/<nom>`.
    pub suggested_destination: String,
}

/// Propositions reçues (événement `sync:collection-offer`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CollectionOffersDto {
    /// L'appareil qui propose.
    pub device: LinkedDeviceDto,
    /// Les collections proposées.
    pub offers: Vec<CollectionOfferDto>,
}

/// Avancement d'un échange de fichiers (événement `sync:collection-progress`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CollectionProgressDto {
    /// Profil local.
    pub profile_id: String,
    /// Partage.
    pub sync_id: String,
    /// Clé de l'appareil.
    pub device_key: String,
    /// Fichiers traités.
    #[ts(type = "number")]
    pub done: u64,
    /// Fichiers à traiter.
    #[ts(type = "number")]
    pub total: u64,
}

/// L'état d'un partage a changé (événement `sync:collection-state`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CollectionStateEventDto {
    /// Profil local.
    pub profile_id: String,
    /// Partage.
    pub sync_id: String,
    /// Son état, absent quand le partage n'existe plus (arrêté, collection retirée).
    pub share: Option<CollectionShareDto>,
}

/// Les copies de conflit d'un partage ont changé (événement `sync:collection-conflicts`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CollectionConflictsDto {
    /// Profil local.
    pub profile_id: String,
    /// Partage.
    pub sync_id: String,
    /// Chemin de la collection sur cet appareil.
    pub root: String,
    /// Les copies de conflit présentes (vide : tout est résolu).
    pub conflicts: Vec<CollectionConflictDto>,
}

/// Ce qu'un dossier contient de synchronisable (commandes `collection_sync_summary` et
/// `collection_sync_inspect_folder`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct FolderSummaryDto {
    /// Le dossier existe.
    pub exists: bool,
    /// Il ne contient rien.
    pub empty: bool,
    /// Nombre de fichiers `.md` qui seraient synchronisés.
    #[ts(type = "number")]
    pub md_files: u64,
    /// Volume de ces fichiers, en octets.
    #[ts(type = "number")]
    pub bytes: u64,
    /// Le dossier est géré par git (un `.git` à sa racine) : avertissement seulement.
    pub has_git: bool,
    /// Les fichiers qui seraient écartés.
    pub ignored: Vec<IgnoredFileDto>,
    /// Le parcours s'arrête à la limite de fichiers ou de profondeur.
    pub truncated: bool,
}
