//! Transferts de fichiers d'une collection : demandes, morceaux, accusés.
//!
//! On demande les fichiers un par un (`FileRequest`) ; l'autre côté répond par des morceaux de
//! 256 Kio (`FileChunk`), ou `FileUnavailable` si la version a changé entre-temps. Une
//! coupure abandonne le fichier en cours : la session suivante échange les manifestes et
//! reprend au premier fichier manquant.

use app::collection_sync::{
    CHUNK_BYTES, FileAction, FileEntry, FileHash, MAX_FILE_BYTES, PlannedFile, SyncPath,
};
use domain::uuid::Uuid;

use super::Session;
use super::collections::Download;
use crate::engine::PeerRuntime;
use crate::error::Result;
use crate::files::{self, Outcome};
use crate::stores::ProfileKey;
use crate::wire::Message;

/// Étape suivante d'un partage.
pub(super) enum Step {
    /// Comparer un manifeste distant au dossier local.
    Evaluate(Vec<FileEntry>),
    /// Traiter le fichier suivant de la file.
    Next,
}

impl<P: ProfileKey> Session<P> {
    /// Enchaîne les étapes d'un partage jusqu'à devoir attendre l'autre appareil (morceaux
    /// d'un fichier) ou la fin de la file.
    pub(super) async fn drive(&mut self, id: Uuid, first: Option<Step>) -> Result<()> {
        let mut step = first;
        while let Some(current) = step.take() {
            step = match current {
                Step::Evaluate(remote) => {
                    self.evaluate(id, remote).await?;
                    self.report_progress(id, true);
                    Some(Step::Next)
                }
                Step::Next => self.next_transfer(id).await?,
            };
        }
        Ok(())
    }

    /// Le fichier suivant de la file : les suppressions se font tout de suite, un fichier à
    /// recevoir est demandé et on attend ses morceaux.
    async fn next_transfer(&mut self, id: Uuid) -> Result<Option<Step>> {
        loop {
            let next = self
                .shares
                .get_mut(&id)
                .and_then(|share| share.queue.pop_front());
            let Some(item) = next else {
                return self.finish_transfers(id).await;
            };
            if item.action == FileAction::DeleteLocal {
                let job = item.clone();
                let result = self
                    .share_job(id, move |ports, ctx| files::delete_local(ports, ctx, &job))
                    .await;
                self.settle(id, &item, result).await?;
                continue;
            }
            let Some(hash) = item.remote else {
                self.settle(id, &item, Ok(Outcome::default())).await?;
                continue;
            };
            let request = Message::FileRequest {
                share: *id.as_bytes(),
                path: item.path.as_str().to_owned(),
                hash: *hash.as_bytes(),
            };
            if let Some(share) = self.shares.get_mut(&id) {
                share.current = Some(Download {
                    item,
                    data: Vec::new(),
                });
            }
            self.send(&request).await?;
            return Ok(None);
        }
    }

    /// Un fichier est traité (écrit, supprimé, ignoré) : historique, accusé, avancement. Un
    /// échec sur ce fichier (disque plein, droits) ne ferme pas la session.
    async fn settle(
        &mut self,
        id: Uuid,
        item: &PlannedFile,
        result: Result<Outcome>,
    ) -> Result<()> {
        let outcome = result.unwrap_or_else(|error| {
            tracing::warn!(path = %item.path, %error, "fichier de collection non traité");
            Outcome::default()
        });
        let Some(share) = self.shares.get_mut(&id) else {
            return Ok(());
        };
        for (path, hash, bytes) in outcome.replaced {
            share.history.remember(&path, hash, bytes);
        }
        share.done += 1;
        share.changed |= outcome.changed;
        self.report_progress(id, true);
        if let Some(hash) = outcome.ack {
            self.send_ack(id, &item.path, hash).await?;
        }
        Ok(())
    }

    async fn send_ack(&mut self, id: Uuid, path: &SyncPath, hash: Option<FileHash>) -> Result<()> {
        self.send(&Message::FileAck {
            share: *id.as_bytes(),
            path: path.as_str().to_owned(),
            hash: hash.map(|hash| *hash.as_bytes()),
        })
        .await
    }

    /// La file est vide : le manifeste à jour part si des fichiers ont changé ici, et un
    /// manifeste distant arrivé entre-temps est évalué.
    async fn finish_transfers(&mut self, id: Uuid) -> Result<Option<Step>> {
        let Some(share) = self.shares.get_mut(&id) else {
            return Ok(None);
        };
        share.current = None;
        let waiting = share.waiting.take();
        let changed = std::mem::take(&mut share.changed);
        self.report_progress(id, false);
        if let Some(remote) = waiting {
            return Ok(Some(Step::Evaluate(remote)));
        }
        if changed {
            self.send_manifest(id, false).await?;
        }
        self.mark_synced(id).await;
        Ok(None)
    }

    /// Date du dernier échange complet, et l'interface relit l'état.
    async fn mark_synced(&self, id: Uuid) {
        let Some(share) = self.shares.get(&id) else {
            return;
        };
        let (remote, now) = (self.remote, self.shared.clock.timestamp());
        let touched = self
            .shared
            .stores
            .with(&share.profile, move |store| {
                store.touch_peer(&id, &remote, &now)
            })
            .await;
        if let Err(error) = touched {
            tracing::warn!(%error, "dernier échange non enregistré");
        }
        self.shared.collection_changed(&share.profile, id);
    }

    pub(super) fn report_progress(&self, id: Uuid, syncing: bool) {
        let Some(share) = self.shares.get(&id) else {
            return;
        };
        let progress = PeerRuntime {
            syncing,
            done: share.done,
            total: share.total,
        };
        self.shared
            .set_progress(&share.profile, id, self.remote, progress);
    }

    /// L'autre appareil demande une version d'un fichier : on la sert par morceaux, depuis le
    /// fichier ou, s'il vient d'être remplacé, depuis l'historique de la session.
    pub(super) async fn on_file_request(
        &mut self,
        share: [u8; 16],
        path: String,
        hash: [u8; 32],
    ) -> Result<()> {
        let id = Uuid::from_bytes(share);
        let wanted = FileHash::from_bytes(hash);
        let bytes = self.version_of(id, &path, wanted).await;
        match bytes {
            Some(bytes) => self.send_file(share, &path, hash, &bytes).await,
            None => {
                self.send(&Message::FileUnavailable { share, path, hash })
                    .await
            }
        }
    }

    async fn version_of(&self, id: Uuid, path: &str, hash: FileHash) -> Option<Vec<u8>> {
        let state = self.shares.get(&id)?;
        if let Some(old) = state.history.find(path, &hash) {
            return Some(old.as_ref().clone());
        }
        let path = SyncPath::parse(path, &self.shared.files.excludes()).ok()?;
        let read = self
            .share_job(id, move |ports, ctx| {
                if !ports.disk.exists(ctx.root, &path) {
                    return Ok(None);
                }
                let bytes = ports.disk.read(ctx.root, &path)?;
                Ok((FileHash::of(&bytes) == hash).then_some(bytes))
            })
            .await;
        read.unwrap_or_else(|error| {
            tracing::debug!(%error, "fichier de collection illisible");
            None
        })
    }

    async fn send_file(
        &mut self,
        share: [u8; 16],
        path: &str,
        hash: [u8; 32],
        bytes: &[u8],
    ) -> Result<()> {
        let total = bytes.len() as u64;
        let chunks: Vec<&[u8]> = if bytes.is_empty() {
            vec![bytes]
        } else {
            bytes.chunks(CHUNK_BYTES).collect()
        };
        let mut offset = 0_u64;
        for chunk in chunks {
            let message = Message::FileChunk {
                share,
                path: path.to_owned(),
                hash,
                total,
                offset,
                data: chunk.to_vec(),
            };
            self.send(&message).await?;
            offset += chunk.len() as u64;
        }
        Ok(())
    }

    /// Un morceau du fichier demandé. Un morceau inattendu, hors ordre ou trop gros abandonne
    /// ce fichier (le prochain manifeste le redemandera).
    pub(super) async fn on_file_chunk(&mut self, chunk: Chunk) -> Result<()> {
        let id = Uuid::from_bytes(chunk.share);
        let Some(download) = self.shares.get_mut(&id).and_then(|s| s.current.as_mut()) else {
            return Ok(());
        };
        let requested = download.item.remote.map(|hash| *hash.as_bytes());
        if download.item.path.as_str() != chunk.path || requested != Some(chunk.hash) {
            return Ok(());
        }
        if !chunk.fits(download.data.len()) {
            return self.abandon(id).await;
        }
        download.data.extend_from_slice(&chunk.data);
        if (download.data.len() as u64) < chunk.total {
            return Ok(());
        }
        let Some(done) = self
            .shares
            .get_mut(&id)
            .and_then(|share| share.current.take())
        else {
            return Ok(());
        };
        let item = done.item.clone();
        let result = self.apply_download(id, done).await;
        self.settle(id, &item, result).await?;
        self.drive(id, Some(Step::Next)).await
    }

    /// Écrit la version reçue (ou la fusionne), dans un thread bloquant.
    async fn apply_download(&self, id: Uuid, download: Download) -> Result<Outcome> {
        let Download { item, data } = download;
        self.share_job(id, move |ports, ctx| match item.action {
            FileAction::Pull => files::pull(ports, ctx, &item, &data),
            FileAction::Reconcile => files::reconcile(ports, ctx, &item, &data),
            _ => Ok(Outcome::default()),
        })
        .await
    }

    async fn abandon(&mut self, id: Uuid) -> Result<()> {
        let current = self
            .shares
            .get_mut(&id)
            .and_then(|share| share.current.take());
        let Some(download) = current else {
            return Ok(());
        };
        self.settle(id, &download.item, Ok(Outcome::default()))
            .await?;
        self.drive(id, Some(Step::Next)).await
    }

    /// La version demandée a changé chez l'autre appareil : on passe au fichier suivant.
    pub(super) async fn on_file_unavailable(
        &mut self,
        share: [u8; 16],
        path: String,
        hash: [u8; 32],
    ) -> Result<()> {
        let id = Uuid::from_bytes(share);
        let matches = self
            .shares
            .get(&id)
            .and_then(|s| s.current.as_ref())
            .is_some_and(|d| {
                d.item.path.as_str() == path && d.item.remote.map(|h| *h.as_bytes()) == Some(hash)
            });
        if matches {
            self.abandon(id).await?;
        }
        Ok(())
    }

    /// L'autre appareil a maintenant la version annoncée : elle devient notre dernière version
    /// échangée.
    pub(super) async fn on_file_ack(
        &mut self,
        share: [u8; 16],
        path: String,
        hash: Option<[u8; 32]>,
    ) -> Result<()> {
        let id = Uuid::from_bytes(share);
        let Ok(path) = SyncPath::parse(&path, &self.shared.files.excludes()) else {
            return Ok(());
        };
        if !self.shares.contains_key(&id) {
            return Ok(());
        }
        let hash = hash.map(FileHash::from_bytes);
        self.share_job(id, move |ports, ctx| files::acked(ports, ctx, &path, hash))
            .await
    }
}

/// Un morceau de fichier reçu.
pub(super) struct Chunk {
    pub share: [u8; 16],
    pub path: String,
    pub hash: [u8; 32],
    pub total: u64,
    pub offset: u64,
    pub data: Vec<u8>,
}

impl Chunk {
    /// Le morceau suit les `received` octets déjà reçus, sans dépasser la taille annoncée, qui
    /// reste dans la limite d'un fichier.
    fn fits(&self, received: usize) -> bool {
        let end = self.offset.saturating_add(self.data.len() as u64);
        self.total <= MAX_FILE_BYTES
            && self.offset == received as u64
            && end <= self.total
            && self.data.len() <= 4 * CHUNK_BYTES
    }
}
