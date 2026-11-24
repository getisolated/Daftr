//! Rattrapage d'un espace, dans chaque sens, sur la même session.
//!
//! L'émetteur X envoie `Changed` (documents écrits depuis le `seq` acquitté par l'autre, par
//! lots de 200, l'espace en premier) ; le pair Y répond `Diffs` (ce que X ignore, et l'état de
//! Y) ; X applique, renvoie `Updates` (ce que Y ignore), puis `Done {seq}` une fois toutes les
//! réponses reçues ; Y, qui a tout appliqué dans l'ordre, répond `Ack {seq}` et X enregistre ce
//! `seq` comme acquitté. Les modifications locales suivent le même chemin, limité aux
//! documents écrits depuis.

use std::collections::HashSet;

use app::AppResult;
use app::ports::sync::{Applied, ChangedDoc, PeerState, RemoteUpdate, SyncEvent, SyncStore};
use app::sync::{StateVector, Update};
use domain::sync::{DocId, SpaceId};

use super::Session;
use crate::error::{Error, Result};
use crate::framing::MAX_MESSAGE_BYTES;
use crate::stores::ProfileKey;
use crate::wire::{self, BATCH_BYTES, BATCH_DOCS, DocDiff, DocState, DocUpdate, Message};

/// Marge pour l'enveloppe d'un message autour d'un document.
const ENVELOPE_BYTES: usize = 4096;

/// Un rattrapage en cours, côté émetteur.
#[derive(Debug, Default)]
pub(super) struct Round {
    /// Dernier `seq` envoyé.
    upto: u64,
    /// Réponses `Diffs` attendues.
    waiting: usize,
    /// `Done` envoyé : seul l'acquittement manque.
    done: bool,
    /// Une modification est arrivée pendant le rattrapage : en refaire un ensuite.
    dirty: bool,
    /// Documents déjà redemandés en entier (incomplets).
    retried: HashSet<DocId>,
}

/// Réponse de l'émetteur à des `Diffs` reçus.
struct Answer {
    applied: Applied,
    updates: Vec<DocUpdate>,
    retry: Vec<DocState>,
}

impl<P: ProfileKey> Session<P> {
    /// Lance le rattrapage d'un espace (ou en note un nouveau s'il en a un en cours).
    pub(super) async fn start_round(&mut self, space: SpaceId) -> Result<()> {
        if let Some(round) = self.rounds.get_mut(&space) {
            round.dirty = true;
            return Ok(());
        }
        let Some(profile) = self.common_profile(&space) else {
            return Ok(());
        };
        let remote = self.remote;
        let changed = self
            .shared
            .stores
            .with(&profile, move |store| {
                let acked = store.peer(&remote)?.map_or(0, |peer| peer.acked_seq);
                store.changed_since(acked)
            })
            .await?;
        let Some(upto) = changed.iter().map(|doc| doc.seq).max() else {
            self.finish_initial(&space, &profile);
            return Ok(());
        };

        let batches: Vec<Message> = changed
            .chunks(BATCH_DOCS)
            .map(|chunk| changed_message(&space, chunk))
            .collect();
        let round = Round {
            upto,
            waiting: batches.len(),
            ..Round::default()
        };
        self.rounds.insert(space, round);
        self.shared.set_syncing(self.id, true);
        for batch in &batches {
            self.send(batch).await?;
        }
        Ok(())
    }

    /// Côté pair : répond à `Changed` par ses diffs. Un espace non partagé ne révèle rien.
    pub(super) async fn on_changed(&mut self, space: [u8; 32], docs: Vec<DocState>) -> Result<()> {
        let space = SpaceId::from_bytes(space);
        let Some(profile) = self.common_profile(&space) else {
            return Ok(());
        };
        let requested = parse_states(docs)?;
        let diffs = self
            .shared
            .stores
            .with(&profile, move |store| diffs_for(store, &requested))
            .await?;

        let mut batches = batch_diffs(diffs);
        let last = batches.len().saturating_sub(1);
        if batches.is_empty() {
            batches.push(Vec::new());
        }
        for (index, docs) in batches.into_iter().enumerate() {
            let space = *space.as_bytes();
            let message = Message::Diffs {
                space,
                docs,
                last: index == last,
            };
            self.send(&message).await?;
        }
        Ok(())
    }

    /// Côté émetteur : applique les diffs du pair et lui renvoie les siens.
    pub(super) async fn on_diffs(
        &mut self,
        space: [u8; 32],
        docs: Vec<DocDiff>,
        last: bool,
    ) -> Result<()> {
        let space = SpaceId::from_bytes(space);
        let Some(profile) = self.common_profile(&space) else {
            return Ok(());
        };
        if !self.rounds.contains_key(&space) {
            return Ok(());
        }
        let received = parse_diffs(docs)?;
        let answer = self
            .shared
            .stores
            .with(&profile, move |store| answer_diffs(store, &received))
            .await?;
        self.applied(&profile, answer.applied);

        for docs in batch_updates(answer.updates) {
            let space = *space.as_bytes();
            self.send(&Message::Updates { space, docs }).await?;
        }
        self.retry_incomplete(&space, answer.retry).await?;
        if last {
            self.reply_received(&space).await?;
        }
        Ok(())
    }

    /// Redemande en entier les documents restés incomplets (une seule fois chacun).
    async fn retry_incomplete(&mut self, space: &SpaceId, states: Vec<DocState>) -> Result<()> {
        let Some(round) = self.rounds.get_mut(space) else {
            return Ok(());
        };
        let states: Vec<DocState> = states
            .into_iter()
            .filter(|state| wire::doc_id(&state.doc).is_ok_and(|doc| round.retried.insert(doc)))
            .collect();
        if states.is_empty() {
            return Ok(());
        }
        round.waiting += 1;
        let space = *space.as_bytes();
        self.send(&Message::Changed {
            space,
            docs: states,
        })
        .await
    }

    /// Une réponse `Diffs` est complète : à la dernière, `Done`.
    async fn reply_received(&mut self, space: &SpaceId) -> Result<()> {
        let Some(round) = self.rounds.get_mut(space) else {
            return Ok(());
        };
        round.waiting = round.waiting.saturating_sub(1);
        if round.waiting > 0 || round.done {
            return Ok(());
        }
        round.done = true;
        let message = Message::Done {
            space: *space.as_bytes(),
            seq: round.upto,
        };
        self.send(&message).await
    }

    /// Côté pair : applique les updates de l'émetteur.
    pub(super) async fn on_updates(&mut self, space: [u8; 32], docs: Vec<DocUpdate>) -> Result<()> {
        let space = SpaceId::from_bytes(space);
        let Some(profile) = self.common_profile(&space) else {
            return Ok(());
        };
        let updates = parse_updates(docs)?;
        let applied = self
            .shared
            .stores
            .with(&profile, move |store| store.apply_remote(&updates))
            .await?;
        self.applied(&profile, applied);
        Ok(())
    }

    /// Côté pair : tout ce qui précède `Done` est appliqué, on acquitte.
    pub(super) async fn on_done(&mut self, space: [u8; 32], seq: u64) -> Result<()> {
        if !self.is_common(&SpaceId::from_bytes(space)) {
            return Ok(());
        }
        self.send(&Message::Ack { space, seq }).await
    }

    /// Côté émetteur : le pair a tout appliqué jusqu'à `seq`.
    pub(super) async fn on_ack(&mut self, space: [u8; 32], seq: u64) -> Result<()> {
        let space = SpaceId::from_bytes(space);
        let Some(profile) = self.local.get(&space).cloned() else {
            return Ok(());
        };
        self.record_ack(&profile, seq).await?;
        if self.snapshots_sent.remove(&space) {
            self.shared.offer_delivered(&self.remote, &profile);
        }

        let finished = self
            .rounds
            .get(&space)
            .is_some_and(|round| round.done && round.upto == seq);
        if !finished {
            return Ok(());
        }
        let dirty = self.rounds.remove(&space).is_some_and(|round| round.dirty);
        self.shared.set_syncing(self.id, !self.rounds.is_empty());
        self.finish_initial(&space, &profile);
        if dirty {
            self.start_round(space).await?;
        }
        Ok(())
    }

    /// Enregistre le curseur du pair, sa dernière adresse et la date de l'échange.
    async fn record_ack(&self, profile: &P, seq: u64) -> Result<()> {
        let (remote, addr) = (self.remote, self.addr);
        let now = self.shared.clock.timestamp();
        self.shared
            .stores
            .with(profile, move |store| {
                let previous = store.peer(&remote)?.unwrap_or_default();
                let state = PeerState {
                    acked_seq: previous.acked_seq.max(seq),
                    last_addr: addr.or(previous.last_addr),
                    last_seen_at: Some(now),
                };
                store.set_peer(&remote, &state)
            })
            .await
    }

    /// Fin de la synchronisation initiale d'un espace rejoint.
    fn finish_initial(&self, space: &SpaceId, profile: &P) {
        if self.shared.state().joining.remove(space) {
            self.shared.publish(SyncEvent::InitialSyncDone {
                profile: profile.clone(),
            });
        }
    }
}

fn changed_message(space: &SpaceId, chunk: &[ChangedDoc]) -> Message {
    let docs = chunk
        .iter()
        .map(|changed| DocState {
            doc: changed.doc.to_string(),
            sv: changed.state_vector.as_bytes().to_vec(),
        })
        .collect();
    Message::Changed {
        space: *space.as_bytes(),
        docs,
    }
}

fn check_batch<T>(docs: &[T]) -> Result<()> {
    if docs.len() > BATCH_DOCS {
        return Err(Error::Malformed);
    }
    Ok(())
}

fn parse_states(docs: Vec<DocState>) -> Result<Vec<(DocId, StateVector)>> {
    check_batch(&docs)?;
    docs.into_iter()
        .map(|state| Ok((wire::doc_id(&state.doc)?, wire::state_vector(state.sv)?)))
        .collect()
}

fn parse_diffs(docs: Vec<DocDiff>) -> Result<Vec<(DocId, Update, StateVector)>> {
    check_batch(&docs)?;
    docs.into_iter()
        .map(|diff| {
            let doc = wire::doc_id(&diff.doc)?;
            Ok((doc, wire::update(diff.update), wire::state_vector(diff.sv)?))
        })
        .collect()
}

fn parse_updates(docs: Vec<DocUpdate>) -> Result<Vec<RemoteUpdate>> {
    check_batch(&docs)?;
    docs.into_iter()
        .map(|update| {
            let doc = wire::doc_id(&update.doc)?;
            Ok(RemoteUpdate {
                doc,
                update: wire::update(update.update),
            })
        })
        .collect()
}

/// Ce que le pair sait et que l'émetteur ignore, avec l'état du pair, document par document.
fn diffs_for(store: &dyn SyncStore, requested: &[(DocId, StateVector)]) -> AppResult<Vec<DocDiff>> {
    requested
        .iter()
        .map(|(doc, remote)| {
            Ok(DocDiff {
                doc: doc.to_string(),
                update: store.diff(doc, remote)?.into_bytes(),
                sv: store.state_vector(doc)?.into_bytes(),
            })
        })
        .collect()
}

/// Applique les diffs reçus en une transaction, puis calcule ce que le pair ignore encore et
/// l'état des documents restés incomplets.
fn answer_diffs(
    store: &dyn SyncStore,
    received: &[(DocId, Update, StateVector)],
) -> AppResult<Answer> {
    let updates: Vec<RemoteUpdate> = received
        .iter()
        .filter(|(_, update, _)| !update.is_empty())
        .map(|(doc, update, _)| RemoteUpdate {
            doc: *doc,
            update: update.clone(),
        })
        .collect();
    let applied = if updates.is_empty() {
        Applied::default()
    } else {
        store.apply_remote(&updates)?
    };

    let mut back = Vec::new();
    for (doc, _, remote) in received {
        let update = store.diff(doc, remote)?;
        if !update.is_empty() {
            back.push(DocUpdate {
                doc: doc.to_string(),
                update: update.into_bytes(),
            });
        }
    }
    let retry = applied
        .incomplete
        .iter()
        .map(|doc| {
            Ok(DocState {
                doc: doc.to_string(),
                sv: store.state_vector(doc)?.into_bytes(),
            })
        })
        .collect::<AppResult<_>>()?;
    Ok(Answer {
        applied,
        updates: back,
        retry,
    })
}

/// Découpe en lots de [`BATCH_DOCS`] documents et d'environ [`BATCH_BYTES`] octets ; un
/// document qui ne tient dans aucun message est écarté (journalisé).
fn batch<T>(items: Vec<T>, size: impl Fn(&T) -> usize) -> Vec<Vec<T>> {
    let mut batches = Vec::new();
    let mut current = Vec::new();
    let mut bytes = 0;
    for item in items {
        let item_bytes = size(&item) + ENVELOPE_BYTES;
        if item_bytes > MAX_MESSAGE_BYTES {
            tracing::warn!(
                bytes = item_bytes,
                "document trop gros pour un message, ignoré"
            );
            continue;
        }
        if !current.is_empty() && (current.len() == BATCH_DOCS || bytes + item_bytes > BATCH_BYTES)
        {
            batches.push(std::mem::take(&mut current));
            bytes = 0;
        }
        bytes += item_bytes;
        current.push(item);
    }
    if !current.is_empty() {
        batches.push(current);
    }
    batches
}

fn batch_diffs(diffs: Vec<DocDiff>) -> Vec<Vec<DocDiff>> {
    batch(diffs, |diff| diff.update.len() + diff.sv.len())
}

/// Lots d'updates (aussi utilisé pour le contenu d'un espace accepté).
pub(super) fn batch_updates(updates: Vec<DocUpdate>) -> Vec<Vec<DocUpdate>> {
    batch(updates, |update| update.update.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn update(bytes: usize) -> DocUpdate {
        DocUpdate {
            doc: "space".to_owned(),
            update: vec![0; bytes],
        }
    }

    #[test]
    fn batches_are_bounded_by_count_and_size() {
        let small: Vec<DocUpdate> = (0..450).map(|_| update(10)).collect();
        let sizes: Vec<usize> = batch_updates(small).iter().map(Vec::len).collect();
        assert_eq!(sizes, [200, 200, 50]);

        let big: Vec<DocUpdate> = (0..3).map(|_| update(BATCH_BYTES / 2)).collect();
        let sizes: Vec<usize> = batch_updates(big).iter().map(Vec::len).collect();
        assert_eq!(sizes, [1, 1, 1]);

        let huge = vec![update(MAX_MESSAGE_BYTES), update(1)];
        let sizes: Vec<usize> = batch_updates(huge).iter().map(Vec::len).collect();
        assert_eq!(sizes, [1]);
    }

    #[test]
    fn oversized_or_invalid_batches_are_malformed() {
        let too_many: Vec<DocUpdate> = (0..=BATCH_DOCS).map(|_| update(1)).collect();
        assert!(matches!(parse_updates(too_many), Err(Error::Malformed)));
        let bad = vec![DocUpdate {
            doc: "note:42".to_owned(),
            update: vec![],
        }];
        assert!(matches!(parse_updates(bad), Err(Error::Malformed)));
    }
}
