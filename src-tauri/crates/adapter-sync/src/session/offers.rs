//! Offres de profils dans la session : l'appareil qui propose envoie `Offer` ; l'autre répond
//! `OfferReply` après le choix de l'utilisateur. Accepté, le proposant ajoute l'appareil à
//! l'espace et envoie tout son contenu (`Snapshot`, `SnapshotEnd`) ; l'autre fait rejoindre
//! l'espace au profil choisi (`adopt_space`), acquitte, puis le rattrapage normal part dans
//! les deux sens (ses propres notes, fusionnées, remontent ainsi).

use app::AppResult;
use app::ports::sync::{ProfileOffer, RemoteSnapshot, SyncEvent, SyncStore};
use app::sync::{StateVector, Update};
use domain::sync::{DeviceKey, DeviceName, DocId, SpaceId};
use domain::uuid::Uuid;

use super::Session;
use super::catchup::batch_updates;
use crate::error::{Error, Result};
use crate::stores::{ProfileKey, in_store};
use crate::wire::{self, BATCH_DOCS, DocUpdate, MAX_OFFERS, Message, OfferEntry};

/// Volume maximal du contenu reçu pour un espace (gardé en mémoire jusqu'à sa fusion).
const MAX_SNAPSHOT_BYTES: usize = 1024 * 1024 * 1024;

/// Contenu d'un espace accepté, en cours de réception.
pub(super) struct Incoming<P> {
    profile: P,
    space: Option<Update>,
    notes: Vec<(Uuid, Update)>,
    bytes: usize,
}

impl<P: ProfileKey> Session<P> {
    /// Envoie les offres en attente pour cet appareil, sauf les espaces qu'il annonce déjà
    /// (être membre ne suffit pas : l'envoi du contenu a pu être interrompu).
    pub(super) async fn send_offers(&mut self) -> Result<()> {
        let pending = self.shared.pending_offers(&self.remote);
        if pending.is_empty() || self.remote_name.is_none() {
            return Ok(());
        }
        let offers = self
            .shared
            .stores
            .run(move |profiles| {
                pending
                    .into_iter()
                    .map(|profile| {
                        let offer = in_store(profiles, &profile, |store| store.offer())?;
                        Ok((profile, offer))
                    })
                    .collect::<AppResult<Vec<_>>>()
            })
            .await?;

        let mut entries = Vec::new();
        for (profile, offer) in offers {
            match offer.filter(|offer| !self.remote_spaces.contains(&offer.space_id)) {
                Some(offer) => {
                    entries.push(wire::offer_entry(&offer));
                    self.offered.insert(offer.space_id, profile);
                }
                None => self.shared.offer_delivered(&self.remote, &profile),
            }
        }
        if entries.is_empty() {
            return Ok(());
        }
        self.send(&Message::Offer(entries)).await
    }

    /// Offre reçue : proposée à l'utilisateur, sauf les espaces déjà synchronisés ici
    /// (refusés d'office : l'appartenance arrivera par le document de l'espace).
    pub(super) async fn on_offer(&mut self, entries: Vec<OfferEntry>) -> Result<()> {
        if entries.len() > MAX_OFFERS {
            return Err(Error::Malformed);
        }
        let offers: Vec<ProfileOffer> = entries.iter().map(wire::profile_offer).collect();
        let spaces: Vec<SpaceId> = offers.iter().map(|offer| offer.space_id).collect();
        let known = self
            .shared
            .stores
            .run(move |profiles| {
                spaces
                    .iter()
                    .map(|space| Ok(profiles.profile_for(space)?.is_some()))
                    .collect::<app::AppResult<Vec<bool>>>()
            })
            .await?;

        let mut fresh = Vec::new();
        for (offer, known) in offers.into_iter().zip(known) {
            let answered = self.shared.accepted_offer(&self.remote, &offer.space_id);
            match (known, answered) {
                // Déjà acceptée sur une connexion précédente : pas de nouvelle question
                (_, Some(profile)) => self.accept_offer(offer.space_id, profile).await?,
                (true, None) => self.reply_offer(offer.space_id, false).await?,
                (false, None) => fresh.push(offer),
            }
        }
        let Some(name) = self.remote_name.clone().filter(|_| !fresh.is_empty()) else {
            return Ok(());
        };
        self.shared.receive_offers(&self.remote, &fresh);
        self.shared.publish(SyncEvent::OfferReceived {
            device: self.remote,
            name,
            offers: fresh,
        });
        Ok(())
    }

    /// L'utilisateur a accepté : le contenu de l'espace ira dans `profile`. Sans effet si la
    /// réception est déjà en cours (offre répétée par une nouvelle connexion).
    pub(super) async fn accept_offer(&mut self, space: SpaceId, profile: P) -> Result<()> {
        if self.accepted.contains_key(&space) {
            return Ok(());
        }
        let incoming = Incoming {
            profile,
            space: None,
            notes: Vec::new(),
            bytes: 0,
        };
        self.accepted.insert(space, incoming);
        self.reply_offer(space, true).await
    }

    /// Répond à une offre.
    pub(super) async fn reply_offer(&mut self, space: SpaceId, accepted: bool) -> Result<()> {
        let space = *space.as_bytes();
        self.send(&Message::OfferReply { space, accepted }).await
    }

    /// Réponse à notre offre : acceptée, on envoie le contenu de l'espace.
    pub(super) async fn on_offer_reply(&mut self, space: [u8; 32], accepted: bool) -> Result<()> {
        let space = SpaceId::from_bytes(space);
        let Some(profile) = self.offered.remove(&space) else {
            return Ok(());
        };
        if !accepted {
            self.shared.offer_delivered(&self.remote, &profile);
            return Ok(());
        }
        self.send_snapshot(space, profile).await
    }

    /// Ajoute l'appareil à l'espace, puis envoie tous les documents du profil.
    async fn send_snapshot(&mut self, space: SpaceId, profile: P) -> Result<()> {
        let remote = self.remote;
        let name = self.remote_name.clone().ok_or(Error::Unexpected)?;
        let now = self.shared.clock.timestamp();
        let (docs, upto) = self
            .shared
            .stores
            .with(&profile, move |store| {
                snapshot_docs(store, &remote, &name, &now)
            })
            .await?;

        let total = docs.len() as u64;
        let bytes = *space.as_bytes();
        for docs in batch_updates(docs) {
            let message = Message::Snapshot {
                space: bytes,
                total,
                docs,
            };
            self.send(&message).await?;
        }
        self.send(&Message::SnapshotEnd {
            space: bytes,
            seq: upto,
        })
        .await?;
        self.snapshots_sent.insert(space);

        // L'appareil est maintenant membre : nouvelle annonce, et l'espace modifié part vers
        // les autres appareils
        self.announce().await?;
        self.shared.request_push(profile, Some(self.id));
        Ok(())
    }

    /// Un morceau du contenu d'un espace accepté.
    pub(super) fn on_snapshot(
        &mut self,
        space: [u8; 32],
        total: u64,
        docs: Vec<DocUpdate>,
    ) -> Result<()> {
        let Some(incoming) = self.accepted.get_mut(&SpaceId::from_bytes(space)) else {
            return Ok(());
        };
        if docs.len() > BATCH_DOCS {
            return Err(Error::Malformed);
        }
        for doc in docs {
            incoming.bytes += doc.update.len();
            if incoming.bytes > MAX_SNAPSHOT_BYTES {
                return Err(Error::TooLarge(incoming.bytes));
            }
            match wire::doc_id(&doc.doc)? {
                DocId::Space => incoming.space = Some(wire::update(doc.update)),
                DocId::Note(uuid) => incoming.notes.push((uuid, wire::update(doc.update))),
            }
        }
        let done = incoming.notes.len() as u64 + u64::from(incoming.space.is_some());
        self.shared.publish(SyncEvent::Progress {
            profile: incoming.profile.clone(),
            done,
            total,
        });
        Ok(())
    }

    /// Fin du contenu : le profil rejoint l'espace, puis le rattrapage normal démarre.
    pub(super) async fn on_snapshot_end(&mut self, space: [u8; 32], seq: u64) -> Result<()> {
        let space_id = SpaceId::from_bytes(space);
        let Some(incoming) = self.accepted.remove(&space_id) else {
            return Ok(());
        };
        let snapshot = RemoteSnapshot {
            space: incoming.space.ok_or(Error::Malformed)?,
            notes: incoming.notes,
        };
        let profile = incoming.profile;
        let applied = self
            .shared
            .stores
            .with(&profile, move |store| {
                store.adopt_space(&space_id, &snapshot)
            })
            .await?;
        self.shared.forget_offer(&self.remote, &space_id);
        self.shared.publish(SyncEvent::NotesSynced {
            profile: profile.clone(),
            notes: applied.notes,
            folders: true,
        });

        {
            let mut state = self.shared.state();
            state.accepted_offers.remove(&(self.remote, space_id));
            state.joining.insert(space_id);
        }
        self.send(&Message::Ack { space, seq }).await?;
        self.announce().await?;
        crate::engine::request_refresh(&self.shared);
        Ok(())
    }
}

/// Ajoute l'appareil à l'espace, puis rend tous les documents du profil en entier et le
/// dernier `seq` écrit.
fn snapshot_docs(
    store: &dyn SyncStore,
    remote: &DeviceKey,
    name: &DeviceName,
    now: &str,
) -> AppResult<(Vec<DocUpdate>, u64)> {
    store.add_device(remote, name, now)?;
    let changed = store.changed_since(0)?;
    let upto = changed.iter().map(|doc| doc.seq).max().unwrap_or(0);
    let docs = changed
        .iter()
        .map(|changed| {
            let update = store.diff(&changed.doc, &StateVector::empty())?;
            Ok(DocUpdate {
                doc: changed.doc.to_string(),
                update: update.into_bytes(),
            })
        })
        .collect::<AppResult<Vec<_>>>()?;
    Ok((docs, upto))
}
