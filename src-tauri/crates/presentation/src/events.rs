//! Noms des événements poussés vers les fenêtres (les mêmes qu'avec Electron).

/// Une autre fenêtre a modifié les notes : relire la liste.
pub const NOTES_CHANGED: &str = "notes:changed";
/// Les réglages ont changé (charge utile : `SettingsDto`).
pub const SETTINGS_CHANGED: &str = "settings:changed";
/// Ouvrir les réglages (menu du tray).
pub const OPEN_SETTINGS: &str = "settings:open";
/// L'overlay vient d'être montré.
pub const OVERLAY_SHOWN: &str = "overlay:shown";
/// Événement des collections (charge utile : `CollectionEventDto`).
pub const COLLECTIONS_EVENT: &str = "collections:event";
/// Fichiers à ouvrir dans des onglets (charge utile : `CollectionFileRefDto[]`).
pub const COLLECTIONS_OPEN: &str = "collections:open";
/// Mise à jour disponible (charge utile : `UpdateInfoDto`).
pub const UPDATE_AVAILABLE: &str = "update:available";
/// Avancement du téléchargement (charge utile : `UpdateProgressDto`).
pub const UPDATE_PROGRESS: &str = "update:progress";
/// Installeur téléchargé et vérifié (charge utile : `UpdateInfoDto`).
pub const UPDATE_DOWNLOADED: &str = "update:downloaded";
/// État du serveur MCP changé (charge utile : `McpStatusDto`).
pub const MCP_STATUS: &str = "mcp:status";
/// Échec du téléchargement (charge utile : message).
pub const UPDATE_DOWNLOAD_FAILED: &str = "update:download-failed";
/// Avancement d'un export en PDF (charge utile : `ExportProgressDto`).
pub const EXPORT_PROGRESS: &str = "export:progress";
/// Le profil actif a changé (ou vient d'être déverrouillé) : la fenêtre se recharge.
pub const PROFILE_CHANGED: &str = "profiles:changed";
/// Notes modifiées par un autre appareil (charge utile : `NotesSyncedDto`).
pub const SYNC_NOTES_SYNCED: &str = "sync:notes-synced";
/// État de la synchronisation (charge utile : `SyncStatusDto`).
pub const SYNC_STATUS: &str = "sync:status";
/// Avancement de la synchronisation initiale (charge utile : `SyncProgressDto`).
pub const SYNC_PROGRESS: &str = "sync:progress";
/// Synchronisation initiale terminée (charge utile : `InitialSyncDoneDto`).
pub const SYNC_INITIAL_DONE: &str = "sync:initial-done";
/// Appareil lié (charge utile : `LinkedDeviceDto`).
pub const SYNC_DEVICE_LINKED: &str = "sync:device-linked";
/// Profil partagé par un appareil, accepté d'office : un clone est créé (charge utile :
/// `ProfileReceivedDto`).
pub const SYNC_PROFILE_RECEIVED: &str = "sync:profile-received";
/// Collections reçues « à configurer » nouvelles ou retirées (charge utile :
/// `CollectionOffersDto`).
pub const SYNC_COLLECTION_OFFER: &str = "sync:collection-offer";
/// Avancement d'un échange de fichiers (charge utile : `CollectionProgressDto`).
pub const SYNC_COLLECTION_PROGRESS: &str = "sync:collection-progress";
/// État d'un partage changé (charge utile : `CollectionStateEventDto`).
pub const SYNC_COLLECTION_STATE: &str = "sync:collection-state";
/// Copies de conflit d'un partage changées (charge utile : `CollectionConflictsDto`).
pub const SYNC_COLLECTION_CONFLICTS: &str = "sync:collection-conflicts";
/// Mode appairage : nouveau code ou fin (charge utile : `PairingStateDto`).
pub const SYNC_PAIRING: &str = "sync:pairing";
