//! Collections partagées dans la session : propositions, manifestes, évaluation.
//!
//! Un partage est actif quand l'appareil distant l'a accepté. Chaque côté envoie son manifeste
//! (au démarrage de la session, puis à chaque changement de ses fichiers) et décide seul de ce
//! qu'il doit recevoir ou supprimer ; les transferts sont dans [`super::transfer`].

mod inbox;

use std::collections::{HashSet, VecDeque};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};

use app::collection_sync::{FileEntry, Manifest, PlannedFile};
use app::ports::collection_sync::{SharePeerState, ShareRecord};
use app::ports::sync::SyncEvent;
use domain::uuid::Uuid;

use super::Session;
use super::transfer::Step;
use crate::error::{Error, Result};
use crate::files::{self, History, LocalSummary, Ports, Snapshot, now_nanos};
use crate::stores::{ProfileKey, in_store};
use crate::wire::{
    FEATURE_COLLECTIONS, MANIFEST_PART, MAX_OFFERED, Message, OfferedCollection, WireEntry,
    file_entry, wire_entry,
};

/// Lignes de manifeste distant au plus (le double de la limite de fichiers parcourus : les
/// suppressions comptent aussi).
const MAX_MANIFEST_ENTRIES: usize = 2 * domain::collection::MAX_SCANNED_FILES;

/// Un fichier en cours de réception.
pub(super) struct Download {
    pub item: PlannedFile,
    pub data: Vec<u8>,
}

/// Un partage actif avec l'appareil distant.
pub(super) struct ShareSession<P> {
    pub profile: P,
    pub root: PathBuf,
    /// Empreinte du dernier manifeste envoyé : un manifeste identique n'est pas renvoyé.
    announced: Option<u64>,
    /// Manifeste distant en cours de réception.
    incoming: Vec<FileEntry>,
    /// Manifeste distant arrivé pendant un échange : évalué ensuite.
    pub waiting: Option<Vec<FileEntry>>,
    /// Décisions pas encore exécutées.
    pub queue: VecDeque<PlannedFile>,
    pub current: Option<Download>,
    pub done: u64,
    pub total: u64,
    /// Des fichiers locaux ont changé pendant l'échange : un manifeste à jour suit.
    pub changed: bool,
    pub history: History,
}

impl<P> ShareSession<P> {
    fn new(profile: P, root: PathBuf) -> Self {
        Self {
            profile,
            root,
            announced: None,
            incoming: Vec::new(),
            waiting: None,
            queue: VecDeque::new(),
            current: None,
            done: 0,
            total: 0,
            changed: false,
            history: History::default(),
        }
    }

    /// Un échange est en cours.
    pub fn busy(&self) -> bool {
        self.current.is_some() || !self.queue.is_empty()
    }
}

impl<P: ProfileKey> Session<P> {
    /// L'appareil distant comprend les messages de collections.
    pub(super) fn collections_enabled(&self) -> bool {
        self.remote_features & FEATURE_COLLECTIONS != 0
    }

    /// Met la liste des partages actifs à jour : propose les nouveaux, démarre les acceptés,
    /// abandonne ceux qui ne le sont plus (appareil retiré, arrêt). Les partages dépendent de
    /// la confiance de l'appareil, pas des profils qu'il partage.
    pub(super) async fn sync_collections(&mut self) -> Result<()> {
        if !self.collections_enabled() {
            return Ok(());
        }
        self.send_pending_list().await?;
        let remote = self.remote;
        let shared = self.shares_with_remote().await?;
        let active: HashSet<Uuid> = shared
            .iter()
            .filter(|(_, share)| share.is_active_with(&remote))
            .map(|(_, share)| share.sync_id)
            .collect();
        self.shares.retain(|id, _| active.contains(id));

        let (mut offers, mut started) = (Vec::new(), Vec::new());
        for (profile, share) in shared {
            let id = share.sync_id;
            match share.peer(&remote).map(|peer| peer.state) {
                Some(SharePeerState::Active) if !self.shares.contains_key(&id) => {
                    let share_session = ShareSession::new(profile, PathBuf::from(&share.root));
                    self.shares.insert(id, share_session);
                    started.push(id);
                }
                Some(SharePeerState::Offered) if self.offered_collections.insert(id) => {
                    offers.push((profile, share));
                }
                _ => {}
            }
        }
        self.send_collection_offers(offers).await?;
        for id in started {
            // L'accord a pu se perdre (session remplacée) : le redire avant le premier manifeste
            // évite que l'autre côté, qui croit encore le partage proposé, le retire
            self.reply_collection(*id.as_bytes(), true).await?;
            self.send_manifest(id, true).await?;
        }
        Ok(())
    }

    /// Les partages, de tous les profils, où l'appareil distant figure (proposé ou actif).
    async fn shares_with_remote(&self) -> Result<Vec<(P, ShareRecord)>> {
        let remote = self.remote;
        self.shared
            .stores
            .run(move |profiles| {
                let mut found = Vec::new();
                for profile in profiles.share_profiles()? {
                    let shares = in_store(profiles, &profile, |store| store.shares())?;
                    for share in shares {
                        if share.peer(&remote).is_some() {
                            found.push((profile.clone(), share));
                        }
                    }
                }
                Ok(found)
            })
            .await
    }

    async fn find_share(&self, id: &Uuid) -> Result<Option<(P, ShareRecord)>> {
        let shared = self.shares_with_remote().await?;
        Ok(shared.into_iter().find(|(_, share)| share.sync_id == *id))
    }

    async fn send_collection_offers(&mut self, offers: Vec<(P, ShareRecord)>) -> Result<()> {
        let mut entries = Vec::new();
        for (profile, share) in offers.into_iter().take(MAX_OFFERED) {
            let root = PathBuf::from(&share.root);
            let counted = self
                .shared
                .with_ports(&profile, move |ports| count_files(ports, &root))
                .await?;
            let Some((files, bytes)) = counted else {
                self.offered_collections.remove(&share.sync_id);
                continue;
            };
            let source = profile.clone();
            let info = self
                .shared
                .stores
                .run(move |profiles| profiles.profile_info(&source))
                .await?;
            entries.push(OfferedCollection {
                // Le profil source n'est désigné que s'il est synchronisé avec cet appareil
                space: self
                    .common_space_of(&profile)
                    .map(|space| *space.as_bytes()),
                profile_id: info.id,
                profile_name: info.name.unwrap_or_default(),
                share: *share.sync_id.as_bytes(),
                name: share.name,
                files,
                bytes,
            });
        }
        if entries.is_empty() {
            return Ok(());
        }
        self.send(&Message::CollectionOffer(entries)).await
    }

    pub(in crate::session) async fn reply_collection(
        &mut self,
        share: [u8; 16],
        accepted: bool,
    ) -> Result<()> {
        self.send(&Message::CollectionReply { share, accepted })
            .await
    }

    /// Réponse de l'appareil distant à notre proposition.
    pub(super) async fn on_collection_reply(
        &mut self,
        share: [u8; 16],
        accepted: bool,
    ) -> Result<()> {
        let id = Uuid::from_bytes(share);
        self.offered_collections.remove(&id);
        let Some((profile, _)) = self.find_share(&id).await? else {
            return Ok(());
        };
        if accepted {
            let remote = self.remote;
            let active = Some(SharePeerState::Active);
            let set =
                move |store: &dyn app::ports::SyncStore| store.set_peer_state(&id, &remote, active);
            self.shared.stores.with(&profile, set).await?;
        } else {
            self.remove_peer(&profile, id).await?;
        }
        self.shared.collection_changed(&profile, id);
        self.sync_collections().await
    }

    /// L'appareil distant arrête de partager la collection, ou ne propose plus celle que nous
    /// n'avions pas encore activée.
    pub(super) async fn on_share_stop(&mut self, share: [u8; 16]) -> Result<()> {
        let id = Uuid::from_bytes(share);
        self.shares.remove(&id);
        self.offered_collections.remove(&id);
        let remote = self.remote;
        let removed = self
            .shared
            .with_inbox(move |inbox| inbox.remove(&remote, &id))
            .await?;
        if removed {
            self.shared.publish(SyncEvent::CollectionPendingChanged);
        }
        if let Some((profile, _)) = self.find_share(&id).await? {
            self.remove_peer(&profile, id).await?;
            self.shared.collection_changed(&profile, id);
        }
        Ok(())
    }

    /// Retire l'appareil distant du partage ; sans plus aucun appareil, le partage s'arrête.
    async fn remove_peer(&self, profile: &P, id: Uuid) -> Result<()> {
        let remote = self.remote;
        self.shared
            .stores
            .with(profile, move |store| {
                store.set_peer_state(&id, &remote, None)?;
                let alone = store
                    .share(&id)?
                    .is_some_and(|share| share.peers.is_empty());
                if alone {
                    store.stop_share(&id)?;
                }
                Ok(())
            })
            .await
    }

    /// Ordre du moteur : des fichiers locaux ont changé.
    pub(super) async fn share_changed(&mut self, id: Uuid) -> Result<()> {
        if self.shares.contains_key(&id) {
            return self.send_manifest(id, false).await;
        }
        Ok(())
    }

    /// Ordre du moteur : l'utilisateur a répondu à une collection proposée.
    pub(super) async fn collection_answer(&mut self, id: Uuid, accepted: bool) -> Result<()> {
        let share = *id.as_bytes();
        self.send(&Message::CollectionReply { share, accepted })
            .await?;
        self.sync_collections().await
    }

    /// Ordre du moteur : le partage est arrêté ici.
    pub(super) async fn stop_share(&mut self, id: Uuid) -> Result<()> {
        self.shares.remove(&id);
        self.offered_collections.remove(&id);
        self.send(&Message::ShareStop {
            share: *id.as_bytes(),
        })
        .await
    }

    /// Parcourt le dossier et envoie le manifeste (sauf s'il n'a pas changé depuis le dernier,
    /// et que `force` n'est pas demandé).
    pub(super) async fn send_manifest(&mut self, id: Uuid, force: bool) -> Result<()> {
        let Some(snapshot) = self.local_snapshot(id).await? else {
            return Ok(());
        };
        let digest = digest_of(&snapshot.manifest);
        let Some(share) = self.shares.get_mut(&id) else {
            return Ok(());
        };
        if !force && share.announced == Some(digest) {
            return Ok(());
        }
        share.announced = Some(digest);

        let entries: Vec<WireEntry> = snapshot.manifest.entries.iter().map(wire_entry).collect();
        let parts: Vec<&[WireEntry]> = entries.chunks(MANIFEST_PART).collect();
        let share = *id.as_bytes();
        if parts.is_empty() {
            let empty = Message::Manifest {
                share,
                entries: Vec::new(),
                last: true,
            };
            return self.send(&empty).await;
        }
        for (index, part) in parts.iter().enumerate() {
            let last = index + 1 == parts.len();
            let message = Message::Manifest {
                share,
                entries: part.to_vec(),
                last,
            };
            self.send(&message).await?;
        }
        Ok(())
    }

    /// L'état local du partage, résumé publié ; `None` si le dossier n'existe plus.
    async fn local_snapshot(&mut self, id: Uuid) -> Result<Option<Snapshot>> {
        let snapshot = self
            .share_job(id, |ports, ctx| files::snapshot(ports, ctx, now_nanos()))
            .await?;
        self.publish_summary(
            id,
            snapshot.as_ref().map(|snapshot| snapshot.summary.clone()),
        );
        Ok(snapshot)
    }

    fn publish_summary(&self, id: Uuid, summary: Option<LocalSummary>) {
        let summary = summary.unwrap_or(LocalSummary {
            root_missing: true,
            ..LocalSummary::default()
        });
        if let Some(share) = self.shares.get(&id)
            && self.shared.set_summary(id, summary)
        {
            self.shared.collection_changed(&share.profile, id);
        }
    }

    /// Manifeste distant reçu (en plusieurs parties) : évalué quand il est complet.
    pub(super) async fn on_manifest(
        &mut self,
        share: [u8; 16],
        entries: Vec<WireEntry>,
        last: bool,
    ) -> Result<()> {
        let id = Uuid::from_bytes(share);
        if !self.shares.contains_key(&id) {
            // Le partage a pu être activé depuis le début de la session
            self.sync_collections().await?;
        }
        if !self.shares.contains_key(&id) {
            // L'autre appareil croit le partage actif : on lui dit qu'il ne l'est plus
            return self.send(&Message::ShareStop { share }).await;
        }
        let excludes = self.shared.files.excludes();
        let Some(state) = self.shares.get_mut(&id) else {
            return Ok(());
        };
        if state.incoming.len() + entries.len() > MAX_MANIFEST_ENTRIES {
            return Err(Error::Malformed);
        }
        let accepted = entries
            .into_iter()
            .filter_map(|entry| file_entry(entry, &excludes));
        state.incoming.extend(accepted);
        if !last {
            return Ok(());
        }
        let remote = std::mem::take(&mut state.incoming);
        if state.busy() {
            state.waiting = Some(remote);
            return Ok(());
        }
        self.drive(id, Some(Step::Evaluate(remote))).await
    }

    /// Compare le dossier local au manifeste distant et met les transferts en file.
    pub(super) async fn evaluate(&mut self, id: Uuid, remote: Vec<FileEntry>) -> Result<()> {
        let evaluation = self
            .share_job(id, move |ports, ctx| files::evaluate(ports, ctx, &remote))
            .await?;
        self.publish_summary(id, evaluation.as_ref().map(|found| found.summary.clone()));
        let Some(evaluation) = evaluation else {
            return Ok(());
        };
        if let Some(share) = self.shares.get_mut(&id) {
            share.total = evaluation.queue.len() as u64;
            share.done = 0;
            share.changed = false;
            share.queue = evaluation.queue.into();
        }
        Ok(())
    }

    /// Exécute `work` sur le disque et la base du partage, dans un thread bloquant.
    pub(super) async fn share_job<T, F>(&self, id: Uuid, work: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&Ports<'_>, &files::ShareCtx<'_>) -> app::AppResult<T> + Send + 'static,
    {
        let share = self.shares.get(&id).ok_or(Error::Unexpected)?;
        let root = share.root.clone();
        let remote = self.remote;
        let role = app::collection_sync::Role::between(&self.shared.identity.key(), &remote);
        let my_name = self.shared.state().device_name.as_str().to_owned();
        self.shared
            .with_ports(&share.profile, move |ports| {
                let ctx = files::ShareCtx {
                    id,
                    root: &root,
                    remote,
                    role,
                    my_name: &my_name,
                };
                work(ports, &ctx)
            })
            .await
    }
}

/// Nombre et volume des fichiers d'une collection, `None` si son dossier n'existe plus.
fn count_files(ports: &Ports<'_>, root: &Path) -> app::AppResult<Option<(u64, u64)>> {
    if !ports.disk.is_directory(root) {
        return Ok(None);
    }
    let scan = ports.disk.scan(root)?;
    let bytes = scan.files.iter().map(|file| file.stat.size).sum();
    Ok(Some((scan.files.len() as u64, bytes)))
}

/// Empreinte de ce qu'un manifeste annonce des fichiers (pas des bases).
fn digest_of(manifest: &Manifest) -> u64 {
    let mut hasher = DefaultHasher::new();
    for entry in &manifest.entries {
        entry.path.as_str().hash(&mut hasher);
        entry.hash.map(|hash| *hash.as_bytes()).hash(&mut hasher);
        entry.size.hash(&mut hasher);
    }
    hasher.finish()
}
