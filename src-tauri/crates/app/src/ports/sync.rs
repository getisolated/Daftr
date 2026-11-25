//! Ports de la synchronisation : stockage CRDT d'un profil, accès aux profils, événements
//! vers l'interface.

use std::hash::Hash;
use std::net::SocketAddr;

use domain::profile::IconFormat;
use domain::sync::{DeviceKey, DeviceName, DocId, SpaceId};
use domain::uuid::Uuid;

use crate::AppResult;
use crate::ports::collection_sync::CollectionSyncStore;
use crate::sync::{ProjectedDevice, StateVector, Update};

/// Un document modifié depuis un curseur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangedDoc {
    /// Le document.
    pub doc: DocId,
    /// Ce que l'appareil en connaît.
    pub state_vector: StateVector,
    /// Numéro local de sa dernière écriture.
    pub seq: u64,
}

/// Une update reçue d'un appareil distant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteUpdate {
    /// Le document visé.
    pub doc: DocId,
    /// L'update, non encore validée.
    pub update: Update,
}

/// Résultat de l'application d'un lot d'updates distantes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Applied {
    /// Notes dont la projection a changé (à rafraîchir dans l'interface).
    pub notes: Vec<Uuid>,
    /// Le document de l'espace a changé (dossiers, appareils ou nom).
    pub space: bool,
    /// Documents refusés (update malformée, trop grosse, incohérente) : le blob stocké reste
    /// intact et le reste du lot est appliqué.
    pub rejected: Vec<DocId>,
    /// Documents qui attendent des opérations antérieures : en redemander un diff complet
    /// depuis leur state vector.
    pub incomplete: Vec<DocId>,
}

/// Curseur d'un appareil distant pour un profil.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PeerState {
    /// Dernier `seq` local acquitté par l'appareil.
    pub acked_seq: u64,
    /// Dernière adresse où il a été joint.
    pub last_addr: Option<SocketAddr>,
    /// Date du dernier échange (format SQLite).
    pub last_seen_at: Option<String>,
}

/// Tout ce que l'appareil distant connaît du profil à fusionner, reçu avant la fusion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteSnapshot {
    /// Le document de l'espace.
    pub space: Update,
    /// Les documents des notes.
    pub notes: Vec<(Uuid, Update)>,
}

/// Stockage CRDT d'un profil synchronisé (blobs, `seq`, curseurs, projection).
///
/// Chaque écriture de blob, locale ou distante, reçoit un nouveau `seq` (compteur local
/// monotone) : c'est ce qui permet la propagation transitive A → B → C.
pub trait SyncStore: CollectionSyncStore + Send + Sync {
    /// L'espace du profil, `None` s'il n'est pas synchronisé.
    fn space_id(&self) -> AppResult<Option<SpaceId>>;
    /// Documents écrits après `seq`, par `seq` croissant, le document `space` en premier.
    fn changed_since(&self, seq: u64) -> AppResult<Vec<ChangedDoc>>;
    /// Ce que l'appareil connaît du document (vide s'il ne le connaît pas).
    fn state_vector(&self, doc: &DocId) -> AppResult<StateVector>;
    /// Ce que l'appareil connaît du document et que `remote` ne connaît pas.
    fn diff(&self, doc: &DocId, remote: &StateVector) -> AppResult<Update>;
    /// Applique un lot d'updates en une transaction : blobs réécrits, `seq` attribués,
    /// projection SQLite (notes, dossiers, notes non résolues reprojetées après l'espace).
    /// Une note purgée qui reçoit une update est recréée.
    fn apply_remote(&self, updates: &[RemoteUpdate]) -> AppResult<Applied>;
    /// Curseur d'un appareil distant.
    fn peer(&self, device: &DeviceKey) -> AppResult<Option<PeerState>>;
    /// Enregistre le curseur d'un appareil distant.
    fn set_peer(&self, device: &DeviceKey, state: &PeerState) -> AppResult<()>;
    /// Appareils de l'espace (membres et révoqués).
    fn devices(&self) -> AppResult<Vec<ProjectedDevice>>;
    /// Ajoute l'appareil à l'espace, ou le réadmet s'il était révoqué (appairage, offre
    /// acceptée) : écriture locale du document `space`, avec un nouveau `seq`. Rend `true` si
    /// le document a changé.
    fn add_device(&self, device: &DeviceKey, name: &DeviceName, now: &str) -> AppResult<bool>;
    /// Révoque l'appareil dans l'espace : il ne peut plus l'échanger. Écriture locale, comme
    /// [`SyncStore::add_device`]. Rend `true` s'il était membre.
    fn revoke_device(&self, device: &DeviceKey, now: &str) -> AppResult<bool>;
    /// Le profil tel qu'on le propose à un autre appareil (espace, nom de l'espace, nombre de
    /// notes non supprimées), `None` s'il n'est pas synchronisé.
    fn offer(&self) -> AppResult<Option<ProfileOffer>>;
    /// Fait rejoindre l'espace au profil (nouveau ou existant), après sauvegarde de la base :
    /// dossiers de même chemin unifiés, dossier par défaut à l'uuid dérivé, notes identiques
    /// dédoublonnées (`sync::merge`). Refusé pour un profil déjà synchronisé.
    fn adopt_space(&self, space_id: &SpaceId, remote: &RemoteSnapshot) -> AppResult<Applied>;
}

/// Un profil synchronisé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncedProfile<P> {
    /// Le profil local.
    pub profile: P,
    /// Son espace.
    pub space_id: SpaceId,
    /// Sa synchronisation est activée.
    pub enabled: bool,
}

/// Ce qu'un profil local montre de lui à un autre appareil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileInfo {
    /// Identifiant local du profil, sous forme de texte.
    pub id: String,
    /// Nom choisi, `None` pour un profil sans nom (le profil par défaut d'une installation
    /// neuve).
    pub name: Option<String>,
    /// C'est le profil par défaut de l'installation (le même sur toutes les installations).
    pub is_default: bool,
}

/// L'icône d'un profil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileIcon {
    /// Format de l'image.
    pub format: IconFormat,
    /// Octets de l'image (déjà vérifiée à l'enregistrement).
    pub bytes: Vec<u8>,
}

/// Accès aux profils de l'installation. Le profil actif passe par sa base déjà ouverte (pour
/// que l'interface reçoive les événements) ; les autres sont ouverts à la demande.
pub trait SyncProfiles: Send + Sync {
    /// Identifiant local d'un profil (jamais échangé).
    type Profile: Clone + Eq + Hash + Send + Sync + 'static;

    /// Le profil local de l'espace.
    fn profile_for(&self, space_id: &SpaceId) -> AppResult<Option<Self::Profile>>;
    /// Exécute `work` sur le stockage du profil, ouvert le temps de l'appel.
    fn with_store(
        &self,
        profile: &Self::Profile,
        work: &mut dyn FnMut(&dyn SyncStore) -> AppResult<()>,
    ) -> AppResult<()>;
    /// Crée un profil cloné d'un profil partagé : `name` absent donne le nom par défaut de
    /// la langue de l'application ; l'icône est enregistrée si elle est donnée. Le profil
    /// créé n'a pas de mot de passe maître.
    fn create_profile(
        &self,
        name: Option<&str>,
        icon: Option<&ProfileIcon>,
    ) -> AppResult<Self::Profile>;
    /// Le profil par défaut de cette installation, s'il existe encore et n'est pas
    /// synchronisé : un profil par défaut partagé par un autre appareil le rejoint au lieu de
    /// créer un clone.
    fn default_profile_to_merge(&self) -> AppResult<Option<Self::Profile>>;
    /// Les profils synchronisés, actifs ou non.
    fn synced_profiles(&self) -> AppResult<Vec<SyncedProfile<Self::Profile>>>;
    /// Tous les profils de l'installation, synchronisés ou non.
    fn profiles(&self) -> AppResult<Vec<Self::Profile>>;
    /// Les profils dont la base liste au moins un partage de collection : les autres ne sont pas
    /// ouverts pour chaque message d'une session. Par défaut, tous les profils.
    fn share_profiles(&self) -> AppResult<Vec<Self::Profile>> {
        self.profiles()
    }
    /// Identifiant et nom d'un profil.
    fn profile_info(&self, profile: &Self::Profile) -> AppResult<ProfileInfo>;
    /// L'icône d'un profil, `None` sans icône.
    fn profile_icon(&self, profile: &Self::Profile) -> AppResult<Option<ProfileIcon>>;
}

/// État affiché par la barre d'état.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncStatus {
    /// Aucun profil synchronisé.
    Disabled,
    /// Pause globale.
    Paused,
    /// À jour avec ce nombre d'appareils connectés.
    UpToDate {
        /// Appareils connectés.
        devices: usize,
    },
    /// Échange en cours.
    Syncing,
    /// Aucun appareil de confiance sur ce réseau.
    NoDevice,
    /// Échec, avec sa cause (clé stable, traduite par l'interface).
    Error {
        /// Cause.
        reason: String,
    },
}

/// Profil proposé par un appareil.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileOffer {
    /// Espace proposé.
    pub space_id: SpaceId,
    /// Nom proposé du profil.
    pub name: String,
    /// Nombre de notes.
    pub note_count: u64,
}

/// Collection proposée par un appareil : une entrée « à configurer » tant que l'utilisateur
/// n'a pas choisi le profil et le dossier qui la reçoivent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionOffer {
    /// Identifiant du partage.
    pub share: Uuid,
    /// Identifiant du profil qui porte la collection chez l'émetteur.
    pub source_profile_id: String,
    /// Nom de ce profil chez l'émetteur (vide : profil sans nom).
    pub source_profile_name: String,
    /// Espace de ce profil, s'il est synchronisé avec cet appareil : le profil local de même
    /// espace reçoit la collection par défaut.
    pub source_space: Option<SpaceId>,
    /// Nom proposé (celui du dossier chez l'émetteur).
    pub name: String,
    /// Nombre de fichiers `.md`.
    pub files: u64,
    /// Volume des fichiers, en octets.
    pub bytes: u64,
}

/// Événement de la synchronisation vers l'interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncEvent<P> {
    /// Des notes d'un profil ont reçu des modifications distantes.
    NotesSynced {
        /// Le profil.
        profile: P,
        /// Les notes modifiées.
        notes: Vec<Uuid>,
        /// Les dossiers ont changé.
        folders: bool,
        /// Nom de l'appareil d'où viennent les modifications, s'il est connu.
        from: Option<DeviceName>,
    },
    /// Nouvel état de la barre d'état.
    Status(SyncStatus),
    /// Avancement de la synchronisation initiale d'un profil.
    Progress {
        /// Le profil.
        profile: P,
        /// Documents traités.
        done: u64,
        /// Documents à traiter.
        total: u64,
    },
    /// Synchronisation initiale d'un profil terminée.
    InitialSyncDone {
        /// Le profil.
        profile: P,
    },
    /// Un appareil vient d'être lié.
    DeviceLinked {
        /// Sa clé.
        device: DeviceKey,
        /// Son nom.
        name: DeviceName,
    },
    /// Un appareil de confiance nous a retirés de ses appareils : nous l'avons retiré à notre
    /// tour (révoqué, retiré des espaces et des partages). Nos notes et fichiers restent.
    DeviceRemoved {
        /// Sa clé.
        device: DeviceKey,
        /// Son nom.
        name: DeviceName,
    },
    /// Un profil partagé par un appareil a été accepté d'office : un nouveau profil local en
    /// est le clone, la synchronisation initiale démarre.
    ProfileReceived {
        /// Le profil créé, ou le profil par défaut existant quand `merged`.
        profile: P,
        /// Son nom (celui du profil d'origine, ou le nom par défaut).
        name: String,
        /// L'appareil qui l'a partagé.
        device: DeviceKey,
        /// Son nom.
        device_name: DeviceName,
        /// Le profil par défaut de l'appareil a été fusionné avec celui d'ici (aucun profil
        /// créé).
        merged: bool,
    },
    /// Un appareil propose des collections : rien n'est écrit avant l'activation. Toutes les
    /// entrées en attente sont relisibles par la commande d'état.
    CollectionOfferReceived {
        /// Sa clé.
        device: DeviceKey,
        /// Son nom.
        name: DeviceName,
        /// Les collections proposées, nouvelles seulement.
        offers: Vec<CollectionOffer>,
    },
    /// Des entrées en attente ont disparu (activées, ignorées, retirées par l'émetteur ou avec
    /// l'appareil) : à relire par la commande d'état.
    CollectionPendingChanged,
    /// Avancement d'un échange de fichiers avec un appareil.
    CollectionProgress {
        /// Le profil.
        profile: P,
        /// Le partage.
        share: Uuid,
        /// L'appareil.
        device: DeviceKey,
        /// Fichiers traités.
        done: u64,
        /// Fichiers à traiter.
        total: u64,
    },
    /// L'état d'un partage a changé (échange terminé, conflit, dossier introuvable…) : à
    /// relire par la commande d'état.
    CollectionChanged {
        /// Le profil.
        profile: P,
        /// Le partage.
        share: Uuid,
    },
}

/// Destinataire des événements de la synchronisation (la fenêtre principale).
pub trait SyncEvents: Send + Sync {
    /// Identifiant local d'un profil.
    type Profile;

    /// Publie un événement.
    fn publish(&self, event: SyncEvent<Self::Profile>);
}
