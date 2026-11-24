//! Les profils de l'installation vus par le moteur de synchronisation (port `SyncProfiles`).
//!
//! Le profil actif passe par sa base déjà ouverte, pour que ses écritures préviennent le
//! moteur et que l'interface voie les changements ; les autres (verrouillés compris : la base
//! n'est pas chiffrée) sont ouverts le temps d'un accès, puis refermés.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use adapter_fs::{DATABASE_FILE, ProfileDirectories, SYNC_FOLDER, SpaceIndexFile};
use adapter_sqlite::{SqliteStore, read_has_shares, read_stored_sync};
use app::folders::FoldersService;
use app::ports::ProfileFiles;
use app::ports::collection_sync::CollectionSyncStore;
use app::ports::sync::{ProfileIcon, ProfileInfo, SyncProfiles, SyncStore, SyncedProfile};
use app::profiles::ProfilesService;
use app::{AppResult, Refusal};
use domain::i18n::{Language, MessageKey, message};
use domain::profile::ProfileId;
use domain::sync::{DeviceKey, DeviceName, SpaceId};
use platform::sync::lock;

use super::index::{SpaceIndex, SpaceOf};

/// Prévenu après chaque écriture locale d'un profil synchronisé (le moteur pousse la
/// modification aux appareils connectés).
pub type ChangeHook = Arc<dyn Fn(&ProfileId) + Send + Sync>;

/// Base du profil actif, déjà ouverte.
struct ActiveStore {
    profile: ProfileId,
    store: Arc<SqliteStore>,
}

/// Les profils pour le moteur, et l'ouverture de toutes les bases de profil de l'app.
pub struct DesktopSyncProfiles {
    root: PathBuf,
    device: DeviceKey,
    profiles: Arc<ProfilesService>,
    active: Mutex<Option<ActiveStore>>,
    index: SpaceIndex,
    hook: Arc<OnceLock<ChangeHook>>,
    /// Langue de l'application : nom par défaut d'un profil cloné dont l'original n'a pas de nom.
    language: Mutex<Language>,
}

impl DesktopSyncProfiles {
    /// Les profils sous `root` (dossier des données), cet appareil étant `device`.
    pub fn new(root: &Path, device: DeviceKey, profiles: Arc<ProfilesService>) -> Self {
        let file = SpaceIndexFile::new(&root.join(SYNC_FOLDER));
        Self {
            root: root.to_path_buf(),
            device,
            profiles,
            active: Mutex::new(None),
            index: SpaceIndex::load(file),
            hook: Arc::new(OnceLock::new()),
            language: Mutex::new(domain::i18n::DEFAULT_LANGUAGE),
        }
    }

    /// Retient la langue de l'application (réglage).
    pub fn set_language(&self, language: Language) {
        *lock(&self.language) = language;
    }

    /// Branche le moteur sur les écritures locales (une seule fois, après sa création).
    pub fn set_change_hook(&self, hook: ChangeHook) {
        if self.hook.set(hook).is_err() {
            tracing::warn!("écouteur des écritures déjà branché");
        }
    }

    /// Ouvre la base d'un profil (créée au besoin avec son dossier), avec la clé de cet
    /// appareil et l'écouteur des écritures : toute base de profil passe par ici.
    pub fn open_store(&self, profile: &ProfileId) -> AppResult<Arc<SqliteStore>> {
        let directories = self.directories();
        directories.create_directory(profile)?;
        let path = directories.directory(profile).join(DATABASE_FILE);
        let store = SqliteStore::open(&path).inspect_err(|error| {
            tracing::error!(%error, path = %path.display(), "base du profil illisible");
        })?;

        let hook = Arc::clone(&self.hook);
        let changed = profile.clone();
        let listener = Arc::new(move || {
            if let Some(hook) = hook.get() {
                hook(&changed);
            }
        });
        Ok(Arc::new(
            store
                .with_device(self.device)
                .with_change_listener(listener),
        ))
    }

    /// Le profil actif et sa base (`None` : verrouillé, sa base sera ouverte à la demande).
    pub fn set_active(&self, profile: ProfileId, store: Option<Arc<SqliteStore>>) {
        *lock(&self.active) = store.map(|store| ActiveStore { profile, store });
    }

    /// Relit l'espace de chaque profil dans sa base, sans l'ouvrir (démarrage).
    pub fn rebuild_index(&self) {
        let directories = self.directories();
        let registry = self.profiles.registry();
        let entries = registry
            .profiles()
            .iter()
            .filter_map(|profile| {
                let path = directories.directory(&profile.id).join(DATABASE_FILE);
                let stored = read_stored_sync(&path).unwrap_or_else(|error| {
                    tracing::warn!(%error, profile = profile.id.as_str(), "espace illisible");
                    None
                })?;
                let space = SpaceOf {
                    space_id: stored.space_id,
                    enabled: stored.enabled,
                };
                Some((profile.id.clone(), space))
            })
            .collect();
        self.index.replace(entries);
    }

    /// Oublie un profil supprimé.
    pub fn forget(&self, profile: &ProfileId) {
        self.index.observe(profile, None);
    }

    /// Le nom d'un profil (`None` : sans nom), ou un refus s'il est inconnu.
    pub fn profile_name(&self, profile: &ProfileId) -> AppResult<Option<String>> {
        Ok(self.profiles.get(profile)?.name)
    }

    /// L'espace d'un profil, s'il est synchronisé.
    pub fn space_of(&self, profile: &ProfileId) -> Option<SpaceOf> {
        self.index.space_of(profile)
    }

    /// Fait du profil un profil synchronisé, activé : crée son espace (nommé `name`) s'il n'en
    /// a pas. Rend l'espace.
    pub fn ensure_space(
        &self,
        profile: &ProfileId,
        name: &str,
        device_name: &DeviceName,
    ) -> AppResult<SpaceId> {
        self.with_sqlite(profile, |store| match store.space_id()? {
            Some(space_id) => {
                store.set_sync_enabled(true)?;
                Ok(space_id)
            }
            None => store.create_space(name, device_name),
        })
    }

    /// Active ou désactive la synchronisation d'un profil déjà synchronisé. L'interface ne
    /// la désactive plus : un profil est synchronisé dès qu'il est partagé.
    #[cfg(test)]
    pub fn set_enabled(&self, profile: &ProfileId, enabled: bool) -> AppResult<()> {
        self.with_sqlite(profile, |store| store.set_sync_enabled(enabled))
    }

    /// Exécute `work` sur la base du profil (celle du profil actif, ou ouverte le temps de
    /// l'appel), puis met à jour le cache des espaces.
    pub fn with_sqlite<T>(
        &self,
        profile: &ProfileId,
        work: impl FnOnce(&SqliteStore) -> AppResult<T>,
    ) -> AppResult<T> {
        let store = self.store_of(profile)?;
        let result = work(&store);
        self.observe(profile, &store);
        result
    }

    /// La base du profil actif, sinon celle du profil (connu) ouverte pour l'occasion.
    pub(super) fn store_of(&self, profile: &ProfileId) -> AppResult<Arc<SqliteStore>> {
        let active = lock(&self.active)
            .as_ref()
            .filter(|active| active.profile == *profile)
            .map(|active| Arc::clone(&active.store));
        if let Some(store) = active {
            return Ok(store);
        }
        if self.profiles.registry().get(profile).is_none() {
            return Err(Refusal::UnknownProfile.into());
        }
        self.open_store(profile)
    }

    fn observe(&self, profile: &ProfileId, store: &SqliteStore) {
        let space = store.space_id().and_then(|space_id| {
            let enabled = store.sync_enabled()?;
            Ok(space_id.map(|space_id| SpaceOf { space_id, enabled }))
        });
        match space {
            Ok(space) => self.index.observe(profile, space),
            Err(error) => tracing::warn!(%error, "espace du profil illisible"),
        }
    }

    fn directories(&self) -> ProfileDirectories {
        ProfileDirectories::new(self.root.clone())
    }
}

impl SyncProfiles for DesktopSyncProfiles {
    type Profile = ProfileId;

    fn profile_for(&self, space_id: &SpaceId) -> AppResult<Option<ProfileId>> {
        Ok(self.index.profile_for(space_id))
    }

    fn with_store(
        &self,
        profile: &ProfileId,
        work: &mut dyn FnMut(&dyn SyncStore) -> AppResult<()>,
    ) -> AppResult<()> {
        self.with_sqlite(profile, |store| work(store))
    }

    /// Le clone d'un profil partagé : `name` absent (profil par défaut sans nom) prend le nom par
    /// défaut dans la langue de l'application ; un nom refusé, ou une icône refusée, laissent le
    /// profil créé sans eux plutôt que d'empêcher la réception. La base est créée avec son
    /// dossier par défaut, comme depuis l'interface.
    fn create_profile(
        &self,
        name: Option<&str>,
        icon: Option<&ProfileIcon>,
    ) -> AppResult<ProfileId> {
        let default_name = message(*lock(&self.language), MessageKey::SyncDefaultProfileName);
        let profile = self.profiles.create(Some(name.unwrap_or(default_name)))?.id;
        if let Some(icon) = icon
            && let Err(error) = self.profiles.set_icon(&profile, &icon.bytes)
        {
            tracing::warn!(%error, profile = profile.as_str(), "icône du profil cloné refusée");
        }
        let store = self.open_store(&profile)?;
        FoldersService::new(store).ensure_default()?;
        tracing::info!(
            profile = profile.as_str(),
            "profil cloné créé pour une synchronisation"
        );
        Ok(profile)
    }

    fn synced_profiles(&self) -> AppResult<Vec<SyncedProfile<ProfileId>>> {
        Ok(self.index.synced())
    }

    fn profiles(&self) -> AppResult<Vec<ProfileId>> {
        Ok(self
            .profiles
            .registry()
            .profiles()
            .iter()
            .map(|profile| profile.id.clone())
            .collect())
    }

    fn share_profiles(&self) -> AppResult<Vec<ProfileId>> {
        let active = lock(&self.active)
            .as_ref()
            .map(|active| (active.profile.clone(), Arc::clone(&active.store)));
        let directories = self.directories();
        let mut found = Vec::new();
        for profile in self.profiles()? {
            let has_shares = match &active {
                Some((id, store)) if *id == profile => !store.shares()?.is_empty(),
                _ => read_has_shares(&directories.directory(&profile).join(DATABASE_FILE))?,
            };
            if has_shares {
                found.push(profile);
            }
        }
        Ok(found)
    }

    fn profile_info(&self, profile: &ProfileId) -> AppResult<ProfileInfo> {
        Ok(ProfileInfo {
            id: profile.as_str().to_owned(),
            name: self.profile_name(profile)?,
        })
    }

    fn profile_icon(&self, profile: &ProfileId) -> AppResult<Option<ProfileIcon>> {
        Ok(self
            .profiles
            .icon(profile)?
            .map(|(format, bytes)| ProfileIcon { format, bytes }))
    }
}
