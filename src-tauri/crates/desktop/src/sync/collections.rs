//! Collections partagées côté application : surveillance des dossiers, état pour l'interface,
//! partage par appareil, activation des collections reçues, nouveau dossier, copies de conflit.
//!
//! Les entrées sont déjà validées par `presentation::collection_sync`. Le moteur
//! (`adapter-sync`) échange les fichiers ; ici on le prévient des changements du disque (un
//! parcours complet au démarrage, puis les événements du système) et on prépare ce que
//! l'interface affiche.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use adapter_fs::{OnChange, ShareDisk, ShareWatcher};
use app::collection_sync::{SyncPath, parse_conflict};
use app::ports::CollectionStore;
use app::ports::collection_sync::{
    CollectionDisk, CollectionSyncStore, PendingCollection, ShareRecord,
};
use app::ports::sync::SyncProfiles;
use app::ports::trust::TrustStore;
use app::{AppResult, Refusal};
use domain::collection::CollectionKind;
use domain::profile::ProfileId;
use domain::settings::AppSettings;
use domain::sync::DeviceKey;
use domain::uuid::Uuid;
use platform::sync::lock;
use presentation::collection_sync::{ConflictKeep, OfferMode, offer_dto};
use presentation::dto::{
    CollectionConflictsDto, CollectionOfferDto, CollectionOffersDto, CollectionShareDto,
    CollectionStateEventDto, CollectionSyncStateDto, FolderSummaryDto, ProfileCollectionDto,
};
use presentation::{IpcError, IpcResult};

use super::actions::blocking;
use super::collection_dto::{ShareView, folder_summary, share_dto, short_key};
use super::{SyncState, ipc_error};

/// Ce que le watcher sait d'un dossier partagé : son profil et son partage.
pub(super) type Routes = Arc<Mutex<HashMap<String, (ProfileId, Uuid)>>>;

/// Les dernières copies de conflit signalées, par partage (pour n'émettre que les changements).
pub(super) type SeenConflicts = Mutex<HashMap<Uuid, Vec<String>>>;

/// L'état d'un partage et, s'il a changé, ses copies de conflit.
pub struct ShareEvents {
    /// `sync:collection-state`.
    pub state: CollectionStateEventDto,
    /// `sync:collection-conflicts`, seulement quand les copies ont changé.
    pub conflicts: Option<CollectionConflictsDto>,
}

impl SyncState {
    /// Surveille les dossiers partagés de `profile` (ceux qui ne le sont plus cessent de l'être)
    /// et prévient le moteur : un parcours complet des fichiers, comme au démarrage.
    pub async fn watch_profile(&self, profile: &ProfileId) {
        let id = profile.clone();
        let shares = self
            .on_profiles(move |profiles| profiles.with_sqlite(&id, |store| store.shares()))
            .await;
        let shares = match shares {
            Ok(shares) => shares,
            Err(error) => {
                tracing::warn!(?error, "partages de collections non lus");
                return;
            }
        };
        self.update_routes(profile, &shares);
        for share in &shares {
            let changed = self
                .engine
                .collection_changed(profile.clone(), share.sync_id)
                .await;
            if let Err(error) = changed {
                tracing::warn!(%error, "dossier partagé non parcouru");
            }
        }
    }

    /// Démarrage : surveille et parcourt les collections partagées de tous les profils (un
    /// partage ne dépend pas de la synchronisation du profil).
    pub async fn start_collections(&self) {
        for profile in self.profiles.share_profiles().unwrap_or_default() {
            self.watch_profile(&profile).await;
        }
    }

    /// Un profil supprimé : ses dossiers ne sont plus surveillés.
    pub(super) fn unwatch_profile(&self, profile: &ProfileId) {
        self.update_routes(profile, &[]);
    }

    fn update_routes(&self, profile: &ProfileId, shares: &[ShareRecord]) {
        let wanted: HashMap<&str, Uuid> = shares
            .iter()
            .map(|share| (share.root.as_str(), share.sync_id))
            .collect();
        let mut routes = lock(&self.routes);
        let stale: Vec<String> = routes
            .iter()
            .filter(|(root, (owner, _))| owner == profile && !wanted.contains_key(root.as_str()))
            .map(|(root, _)| root.clone())
            .collect();
        for root in stale {
            routes.remove(&root);
            self.with_watcher(|watcher| watcher.unwatch(&root));
        }
        for (root, id) in wanted {
            let route = (profile.clone(), id);
            if routes.insert(root.to_owned(), route).is_none() {
                self.with_watcher(|watcher| watcher.watch(root));
            }
        }
    }

    /// Le thread de surveillance ne démarre qu'avec le premier dossier partagé.
    fn with_watcher(&self, work: impl FnOnce(&ShareWatcher)) {
        let mut slot = lock(&self.watcher);
        if slot.is_none() {
            *slot = self.start_watcher();
        }
        if let Some(watcher) = slot.as_ref() {
            work(watcher);
        }
    }

    fn start_watcher(&self) -> Option<ShareWatcher> {
        let routes = Arc::clone(&self.routes);
        let engine = self.engine.clone();
        let runtime = self.runtime.clone();
        let on_change: OnChange = Arc::new(move |root| {
            let route = lock(&routes).get(root).cloned();
            let Some((profile, share)) = route else {
                return;
            };
            let engine = engine.clone();
            runtime.spawn(async move {
                if let Err(error) = engine.collection_changed(profile, share).await {
                    tracing::warn!(%error, "changement de dossier partagé non traité");
                }
            });
        });
        ShareWatcher::start(on_change)
            .inspect_err(|error| tracing::error!(%error, "surveillance des partages impossible"))
            .ok()
    }

    /// Les collections et partages (de `only` si un profil est donné, sinon de tous les profils,
    /// actifs ou non), les entrées « à configurer » et le dossier des collections reçues.
    pub async fn collection_state(
        &self,
        only: Option<ProfileId>,
        settings: &AppSettings,
    ) -> IpcResult<CollectionSyncStateDto> {
        let (names, my_name) = (self.device_names().await?, self.my_name(settings));
        let profiles = match only {
            Some(profile) => {
                self.profiles.profile_name(&profile)?;
                vec![profile]
            }
            None => self.profiles.profiles()?,
        };
        let (mut collections, mut shares) = (Vec::new(), Vec::new());
        for profile in profiles {
            let id = profile.clone();
            let (records, listed) = self
                .on_profiles(move |profiles| {
                    profiles.with_sqlite(&id, |store| Ok((store.shares()?, store.all()?)))
                })
                .await?;
            for collection in listed
                .into_iter()
                .filter(|collection| collection.kind == CollectionKind::Folder)
            {
                let sync_id = records
                    .iter()
                    .find(|record| record.root == collection.path)
                    .map(|record| record.sync_id.to_string());
                collections.push(ProfileCollectionDto {
                    profile_id: profile.as_str().to_owned(),
                    root: collection.path,
                    name: collection.display_name,
                    sync_id,
                });
            }
            for record in records {
                shares.push(self.share_dto_of(&profile, &record, &names, &my_name));
            }
        }
        let received = self.received_folder(settings);
        let offers = self.offer_dtos(&names, &received).await?;
        Ok(CollectionSyncStateDto {
            collections,
            shares,
            offers,
            received_folder: received,
        })
    }

    /// Les entrées « à configurer » pour le renderer, avec le profil local déjà synchronisé
    /// avec leur profil source.
    async fn offer_dtos(
        &self,
        names: &HashMap<DeviceKey, String>,
        received: &str,
    ) -> IpcResult<Vec<CollectionOfferDto>> {
        let pending = self.engine.pending_collections().await.map_err(ipc_error)?;
        pending
            .iter()
            .map(|entry| self.offer_of(entry, names, received))
            .collect()
    }

    fn offer_of(
        &self,
        entry: &PendingCollection,
        names: &HashMap<DeviceKey, String>,
        received: &str,
    ) -> IpcResult<CollectionOfferDto> {
        let synced_here = match entry.offer.source_space {
            Some(space) => self.profiles.profile_for(&space)?,
            None => None,
        };
        let name = names
            .get(&entry.device)
            .cloned()
            .unwrap_or_else(|| short_key(&entry.device));
        Ok(offer_dto(
            &entry.device,
            &name,
            &entry.offer,
            synced_here.as_ref(),
            received,
        ))
    }

    /// `sync:collection-offer` : les nouvelles propositions (`added`) et toutes celles qui
    /// attendent. Les noms d'appareil et le dossier des collections reçues sont ceux de
    /// maintenant.
    pub async fn pending_event(
        &self,
        added: &[(DeviceKey, Uuid)],
        settings: &AppSettings,
    ) -> IpcResult<CollectionOffersDto> {
        let names = self.device_names().await?;
        let received = self.received_folder(settings);
        let pending = self.offer_dtos(&names, &received).await?;
        let added = added
            .iter()
            .filter_map(|added| {
                pending
                    .iter()
                    .find(|offer| {
                        offer.device.key == added.0.to_string()
                            && offer.sync_id == added.1.to_string()
                    })
                    .cloned()
            })
            .collect();
        Ok(CollectionOffersDto { added, pending })
    }

    fn share_dto_of(
        &self,
        profile: &ProfileId,
        record: &ShareRecord,
        names: &HashMap<DeviceKey, String>,
        my_name: &str,
    ) -> CollectionShareDto {
        let devices: Vec<DeviceKey> = record.peers.iter().map(|peer| peer.device).collect();
        let status = self.engine.collection_status(&record.sync_id, &devices);
        share_dto(ShareView {
            profile,
            record,
            status,
            names,
            my_name,
        })
    }

    /// L'état d'un partage après un changement, pour les événements ; `None` du partage s'il
    /// n'existe plus.
    pub async fn share_events(
        &self,
        profile: &ProfileId,
        share: Uuid,
        settings: &AppSettings,
    ) -> IpcResult<ShareEvents> {
        let (names, my_name) = (self.device_names().await?, self.my_name(settings));
        let id = profile.clone();
        let record = self
            .on_profiles(move |profiles| profiles.with_sqlite(&id, |store| store.share(&share)))
            .await?;
        let dto = record
            .as_ref()
            .map(|record| self.share_dto_of(profile, record, &names, &my_name));
        let conflicts = self.changed_conflicts(profile, share, record.as_ref(), dto.as_ref());
        let state = CollectionStateEventDto {
            profile_id: profile.as_str().to_owned(),
            sync_id: share.to_string(),
            share: dto,
        };
        Ok(ShareEvents { state, conflicts })
    }

    /// Les copies de conflit, seulement si elles ne sont plus celles du dernier événement.
    fn changed_conflicts(
        &self,
        profile: &ProfileId,
        share: Uuid,
        record: Option<&ShareRecord>,
        dto: Option<&CollectionShareDto>,
    ) -> Option<CollectionConflictsDto> {
        let current: Vec<String> = dto
            .map(|dto| dto.conflicts.iter().map(|c| c.path.clone()).collect())
            .unwrap_or_default();
        let previous = lock(&self.seen_conflicts).insert(share, current.clone());
        if previous.unwrap_or_default() == current {
            return None;
        }
        Some(CollectionConflictsDto {
            profile_id: profile.as_str().to_owned(),
            sync_id: share.to_string(),
            root: record.map(|record| record.root.clone()).unwrap_or_default(),
            conflicts: dto.map(|dto| dto.conflicts.clone()).unwrap_or_default(),
        })
    }

    /// Nom de chaque appareil de confiance.
    async fn device_names(&self) -> IpcResult<HashMap<DeviceKey, String>> {
        let trust = Arc::clone(&self.trust);
        let devices = blocking(move || trust.devices()).await?;
        Ok(devices
            .into_iter()
            .map(|device| (device.key, device.name.as_str().to_owned()))
            .collect())
    }

    fn my_name(&self, settings: &AppSettings) -> String {
        self.device_name(settings).as_str().to_owned()
    }

    /// Ce qu'un dossier contient de synchronisable (avant de le partager, ou comme destination).
    pub async fn folder_summary(&self, path: PathBuf) -> FolderSummaryDto {
        let disk = Arc::clone(&self.files);
        tokio::task::spawn_blocking(move || folder_summary(&disk, &path))
            .await
            .unwrap_or_else(|_| folder_summary(&ShareDisk::default(), Path::new("")))
    }

    /// « Synchroniser avec mes appareils » : propose la collection `root` de `profile` aux
    /// appareils de confiance choisis (ils n'ont pas besoin de partager le profil).
    pub async fn share_collection(
        &self,
        profile: ProfileId,
        root: String,
        devices: Vec<DeviceKey>,
        settings: &AppSettings,
    ) -> IpcResult<CollectionShareDto> {
        let record = self
            .engine
            .share_collection(profile.clone(), root, devices)
            .await
            .map_err(ipc_error)?;
        self.watch_profile(&profile).await;
        let (names, my_name) = (self.device_names().await?, self.my_name(settings));
        Ok(self.share_dto_of(&profile, &record, &names, &my_name))
    }

    /// Partage la collection `root` avec un seul appareil (`shared`), ou l'en retire. Rend le
    /// partage tel qu'il est ensuite, `None` quand plus aucun appareil ne le partage.
    pub async fn set_collection_device(
        &self,
        profile: ProfileId,
        root: String,
        device: DeviceKey,
        shared: bool,
        settings: &AppSettings,
    ) -> IpcResult<Option<CollectionShareDto>> {
        self.profiles.profile_name(&profile)?;
        let record = self
            .engine
            .set_collection_device(profile.clone(), root, device, shared)
            .await
            .map_err(ipc_error)?;
        self.watch_profile(&profile).await;
        let (names, my_name) = (self.device_names().await?, self.my_name(settings));
        Ok(record.map(|record| self.share_dto_of(&profile, &record, &names, &my_name)))
    }

    /// « Arrêter de synchroniser cette collection » : les fichiers restent des deux côtés.
    pub async fn stop_collection(&self, profile: ProfileId, share: Uuid) -> IpcResult<()> {
        self.engine
            .stop_collection(profile.clone(), share)
            .await
            .map_err(ipc_error)?;
        lock(&self.seen_conflicts).remove(&share);
        self.watch_profile(&profile).await;
        Ok(())
    }

    /// Active une collection reçue dans `profile` (actif ou non) : rien n'est écrit avant. `merge`
    /// fusionne avec un dossier qui contient déjà des fichiers ; refusé sinon.
    pub async fn activate_collection(
        &self,
        device: DeviceKey,
        share: Uuid,
        profile: ProfileId,
        destination: PathBuf,
        mode: OfferMode,
    ) -> IpcResult<()> {
        self.profiles.profile_name(&profile)?;
        self.engine
            .activate_collection(
                device,
                share,
                profile.clone(),
                destination,
                mode == OfferMode::Merge,
            )
            .await
            .map_err(ipc_error)?;
        self.watch_profile(&profile).await;
        Ok(())
    }

    /// Ignore une collection reçue.
    pub async fn ignore_collection(&self, device: DeviceKey, share: Uuid) -> IpcResult<()> {
        self.engine
            .ignore_collection(device, share)
            .await
            .map_err(ipc_error)
    }

    /// Le dossier d'une collection partagée a été relié à un autre emplacement (déjà enregistré
    /// par la liste des collections) : l'historique des échanges est oublié, le prochain échange
    /// repart de zéro (rien n'est supprimé chez l'autre appareil à cause du changement de
    /// dossier).
    pub async fn relinked(&self, profile: &ProfileId, share: Uuid) -> IpcResult<()> {
        let id = profile.clone();
        self.on_profiles(move |profiles| {
            profiles.with_sqlite(&id, |store| store.reset_files(&share))
        })
        .await?;
        self.watch_profile(profile).await;
        Ok(())
    }

    /// `root` est une collection du profil.
    pub async fn has_collection(&self, profile: &ProfileId, root: String) -> IpcResult<bool> {
        let id = profile.clone();
        self.on_profiles(move |profiles| {
            profiles.with_sqlite(&id, |store| Ok(store.get(&root)?.is_some()))
        })
        .await
    }

    /// Le partage de la collection `root`, s'il y en a un.
    pub async fn share_of(
        &self,
        profile: &ProfileId,
        root: String,
    ) -> IpcResult<Option<ShareRecord>> {
        let id = profile.clone();
        self.on_profiles(move |profiles| profiles.with_sqlite(&id, |store| store.share_of(&root)))
            .await
    }

    /// Choisit entre les deux versions d'une copie de conflit : la version gardée est à la place
    /// de l'original, la copie part à la corbeille. Les deux changements suivent le partage
    /// comme n'importe quelle modification.
    pub async fn resolve_conflict(
        &self,
        profile: ProfileId,
        root: String,
        path: String,
        keep: ConflictKeep,
        settings: &AppSettings,
    ) -> IpcResult<()> {
        let share = self
            .share_of(&profile, root.clone())
            .await?
            .ok_or(Refusal::UnknownShare)?;
        let plan = conflict_plan(&self.files.excludes(), &path, keep, &self.my_name(settings))?;
        let disk = Arc::clone(&self.files);
        let root_path = PathBuf::from(root);
        blocking(move || apply_conflict_choice(&disk, &root_path, &plan)).await?;
        self.engine
            .collection_changed(profile, share.sync_id)
            .await
            .map_err(ipc_error)
    }
}

/// Ce qu'il faut faire d'une copie de conflit.
struct ConflictPlan {
    copy: SyncPath,
    original: SyncPath,
    /// La version de la copie remplace l'original.
    copy_wins: bool,
}

/// Quel fichier garder : la copie garde la version de l'appareil dont elle porte le nom.
fn conflict_plan(
    excludes: &[String],
    path: &str,
    keep: ConflictKeep,
    my_name: &str,
) -> IpcResult<ConflictPlan> {
    let invalid = |reason: &'static str| IpcError::invalid_input(reason);
    let copy = SyncPath::parse(path, excludes).map_err(|_| invalid("invalid path"))?;
    let name = parse_conflict(copy.as_str()).ok_or_else(|| invalid("not a conflict copy"))?;
    let original =
        SyncPath::parse(&name.original, excludes).map_err(|_| invalid("invalid path"))?;
    let copy_is_mine = name.is_from(my_name);

    Ok(ConflictPlan {
        copy,
        original,
        copy_wins: (keep == ConflictKeep::Mine) == copy_is_mine,
    })
}

fn apply_conflict_choice(disk: &ShareDisk, root: &Path, plan: &ConflictPlan) -> AppResult<()> {
    if !disk.exists(root, &plan.copy) {
        return Err(Refusal::UnknownCollection.into());
    }
    if plan.copy_wins {
        let bytes = disk.read(root, &plan.copy)?;
        disk.write(root, &plan.original, &bytes)?;
    }
    disk.trash(root, &plan.copy)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(path: &str, keep: ConflictKeep) -> ConflictPlan {
        conflict_plan(&[], path, keep, "Bureau").unwrap()
    }

    #[test]
    fn keeping_mine_uses_the_copy_only_when_it_holds_my_version() {
        let mine = "plan (conflit Bureau).md";
        let theirs = "plan (conflit Portable).md";

        assert!(plan(mine, ConflictKeep::Mine).copy_wins);
        assert!(!plan(mine, ConflictKeep::Other).copy_wins);
        assert!(!plan(theirs, ConflictKeep::Mine).copy_wins);
        assert!(plan(theirs, ConflictKeep::Other).copy_wins);
        assert_eq!(
            plan(theirs, ConflictKeep::Mine).original.as_str(),
            "plan.md"
        );
    }

    #[test]
    fn only_a_valid_conflict_copy_can_be_resolved() {
        let refuse = |path: &str| conflict_plan(&[], path, ConflictKeep::Mine, "Bureau").is_err();

        assert!(refuse("plan.md"));
        assert!(refuse("../plan (conflit Bureau).md"));
        assert!(refuse("plan (conflit Bureau).txt"));
        assert!(refuse("(conflit Bureau).md"));
    }
}
