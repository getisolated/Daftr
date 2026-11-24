//! Ports de la synchronisation des collections : état par profil (partages, appareils, base de
//! chaque fichier) et accès au disque d'une collection partagée.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use domain::sync::DeviceKey;
use domain::uuid::Uuid;

use crate::AppResult;
use crate::collection_sync::{FileHash, Ignored, SyncPath};

/// Où en est un appareil pour un partage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SharePeerState {
    /// La collection lui a été proposée, il n'a pas répondu.
    Offered,
    /// Il l'a acceptée : on échange les fichiers.
    Active,
}

/// Un appareil avec lequel une collection est partagée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharePeer {
    /// Sa clé.
    pub device: DeviceKey,
    /// Proposé ou actif.
    pub state: SharePeerState,
    /// Dernier échange complet (format SQLite).
    pub last_sync_at: Option<String>,
}

/// Une collection partagée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareRecord {
    /// Identifiant commun aux appareils (le chemin reste local).
    pub sync_id: Uuid,
    /// Chemin local de la collection.
    pub root: String,
    /// Nom affiché.
    pub name: String,
    /// Les appareils.
    pub peers: Vec<SharePeer>,
}

impl ShareRecord {
    /// L'état d'un appareil pour ce partage.
    pub fn peer(&self, device: &DeviceKey) -> Option<&SharePeer> {
        self.peers.iter().find(|peer| peer.device == *device)
    }

    /// `device` échange les fichiers de ce partage.
    pub fn is_active_with(&self, device: &DeviceKey) -> bool {
        self.peer(device)
            .is_some_and(|peer| peer.state == SharePeerState::Active)
    }
}

/// Dernier état connu d'un fichier local : évite de relire un fichier qui n'a pas bougé
/// (même taille, même date de modification).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexRow {
    /// Chemin relatif.
    pub path: String,
    /// Empreinte du contenu.
    pub hash: FileHash,
    /// Taille.
    pub size: u64,
    /// Date de modification (nanosecondes depuis l'epoch Unix).
    pub mtime: i64,
}

/// Dernière version échangée d'un fichier avec un appareil, et son texte (fusion à trois
/// voies) quand il est court et valide.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaseRow {
    /// Empreinte de la version.
    pub hash: FileHash,
    /// Le texte de cette version, s'il est gardé.
    pub text: Option<Vec<u8>>,
}

/// État de la synchronisation des collections dans la base d'un profil.
pub trait CollectionSyncStore: Send + Sync {
    /// Tous les partages du profil.
    fn shares(&self) -> AppResult<Vec<ShareRecord>>;
    /// Un partage.
    fn share(&self, sync_id: &Uuid) -> AppResult<Option<ShareRecord>>;
    /// Le partage de la collection `root`.
    fn share_of(&self, root: &str) -> AppResult<Option<ShareRecord>>;
    /// Partage la collection `root` (déjà listée) : lui donne un identifiant si besoin et
    /// propose la collection à `devices` (un appareil déjà actif le reste).
    fn create_share(
        &self,
        root: &str,
        new_id: &Uuid,
        devices: &[DeviceKey],
    ) -> AppResult<ShareRecord>;
    /// Une collection reçue : l'ajoute à la liste sous `root` (si elle n'y est pas) et la
    /// partage avec `device`, déjà actif. Refusé si `root` est déjà partagée sous un autre
    /// identifiant.
    fn accept_share(
        &self,
        sync_id: &Uuid,
        root: &str,
        name: &str,
        device: &DeviceKey,
    ) -> AppResult<ShareRecord>;
    /// Change l'état d'un appareil pour un partage ; `None` le retire (le partage reste).
    fn set_peer_state(
        &self,
        sync_id: &Uuid,
        device: &DeviceKey,
        state: Option<SharePeerState>,
    ) -> AppResult<()>;
    /// Date du dernier échange complet avec l'appareil.
    fn touch_peer(&self, sync_id: &Uuid, device: &DeviceKey, now: &str) -> AppResult<()>;
    /// Arrête le partage : appareils, index et bases oubliés. La collection et ses fichiers
    /// restent.
    fn stop_share(&self, sync_id: &Uuid) -> AppResult<()>;
    /// Retire l'appareil de tous les partages (appareil retiré ou révoqué du profil).
    fn drop_device(&self, device: &DeviceKey) -> AppResult<()>;

    /// Oublie l'index et toutes les dernières versions échangées : le dossier est relié
    /// ailleurs, le prochain échange repart de zéro (aucune suppression déduite).
    fn reset_files(&self, sync_id: &Uuid) -> AppResult<()>;

    /// Index des fichiers locaux d'un partage, par chemin.
    fn file_index(&self, sync_id: &Uuid) -> AppResult<HashMap<String, IndexRow>>;
    /// Remplace l'index d'un partage.
    fn replace_file_index(&self, sync_id: &Uuid, rows: &[IndexRow]) -> AppResult<()>;
    /// Met à jour l'index d'un fichier (écrit par la synchronisation elle-même).
    fn put_index_row(&self, sync_id: &Uuid, row: &IndexRow) -> AppResult<()>;
    /// Retire un fichier de l'index.
    fn drop_index_row(&self, sync_id: &Uuid, path: &str) -> AppResult<()>;
    /// Dernières versions échangées avec un appareil, par chemin.
    fn bases(&self, sync_id: &Uuid, device: &DeviceKey) -> AppResult<HashMap<String, FileHash>>;
    /// La dernière version échangée d'un fichier, avec son texte.
    fn base(&self, sync_id: &Uuid, device: &DeviceKey, path: &str) -> AppResult<Option<BaseRow>>;
    /// Retient la dernière version échangée d'un fichier.
    fn set_base(
        &self,
        sync_id: &Uuid,
        device: &DeviceKey,
        path: &str,
        base: &BaseRow,
    ) -> AppResult<()>;
    /// Oublie la dernière version échangée d'un fichier.
    fn clear_base(&self, sync_id: &Uuid, device: &DeviceKey, path: &str) -> AppResult<()>;
}

/// Taille et date de modification d'un fichier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStat {
    /// Taille en octets.
    pub size: u64,
    /// Date de modification (nanosecondes depuis l'epoch Unix).
    pub mtime: i64,
}

/// Un fichier markdown trouvé dans une collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedFile {
    /// Chemin relatif validé.
    pub path: SyncPath,
    /// Taille et date.
    pub stat: FileStat,
}

/// Résultat du parcours d'une collection partagée.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LocalScan {
    /// Les fichiers à synchroniser.
    pub files: Vec<ScannedFile>,
    /// Les fichiers écartés (trop gros, nom invalide, illisible).
    pub ignored: Vec<Ignored>,
    /// Le parcours s'est arrêté à la limite de fichiers ou de profondeur.
    pub truncated: bool,
}

/// Fichiers d'une collection partagée sur le disque. Aucun chemin n'en sort : chaque méthode
/// reçoit la racine et un [`SyncPath`] déjà validé, et ne suit jamais un lien symbolique.
pub trait CollectionDisk: Send + Sync {
    /// Les dossiers ignorés (réglage des collections).
    fn excludes(&self) -> Vec<String>;
    /// La racine existe et est un dossier.
    fn is_directory(&self, root: &Path) -> bool;
    /// La racine existe et ne contient aucun fichier ni dossier visible.
    fn is_empty_directory(&self, root: &Path) -> bool;
    /// Crée le dossier s'il manque (parents compris) et rend son chemin canonique.
    fn prepare_directory(&self, root: &Path) -> AppResult<PathBuf>;
    /// Parcourt la collection : les `.md`, hors dossiers exclus, de moins de 10 Mio.
    fn scan(&self, root: &Path) -> AppResult<LocalScan>;
    /// Le fichier existe (sans suivre un lien symbolique).
    fn exists(&self, root: &Path, path: &SyncPath) -> bool;
    /// Lit un fichier (10 Mio au plus).
    fn read(&self, root: &Path, path: &SyncPath) -> AppResult<Vec<u8>>;
    /// Écrit un fichier : fichier temporaire voisin puis renommage, dossiers créés au besoin.
    fn write(&self, root: &Path, path: &SyncPath, bytes: &[u8]) -> AppResult<FileStat>;
    /// Envoie un fichier à la corbeille du système.
    fn trash(&self, root: &Path, path: &SyncPath) -> AppResult<()>;
}
