//! Offres de profils dans la session : l'appareil qui partage un profil envoie `Offer` ;
//! l'autre l'accepte d'office (`OfferReply`) sans rien demander à l'utilisateur. Le proposant
//! envoie alors le nom et l'icône (`ProfileInfo`), ajoute l'appareil à l'espace et envoie tout
//! le contenu (`Snapshot`, `SnapshotEnd`) ; l'autre crée un NOUVEAU profil, clone du profil
//! d'origine, qui rejoint l'espace (`adopt_space`), acquitte, puis le rattrapage normal part
//! dans les deux sens. Exception : le profil par défaut (`ProfileInfo::is_default`) est le
//! même partout ; s'il existe ici sans espace, c'est lui qui rejoint l'espace, sans clone.
//!
//! Le profil n'est créé qu'à la fin du contenu : une réception interrompue ne laisse aucun
//! profil vide, et le proposant, qui garde son offre tant que l'espace n'est pas annoncé,
//! recommence à la reconnexion. Un espace déjà présent ici n'est pas recréé (un profil par
//! espace et par appareil).

use app::AppResult;
use app::ports::sync::{ProfileIcon, ProfileOffer, RemoteSnapshot, SyncEvent, SyncStore};
use app::sync::{StateVector, Update};
use domain::profile::validate_icon;
use domain::sync::{DeviceKey, DocId, SpaceId};
use domain::uuid::Uuid;

use super::Session;
use super::catchup::batch_updates;
use crate::error::{Error, Result};
use crate::stores::{ProfileKey, in_store, is_member};
use crate::wire::{
    self, BATCH_DOCS, DocUpdate, FEATURE_PROFILE_INFO, MAX_OFFERS, Message, OfferEntry,
    ProfileInfoEntry,
};

/// Volume maximal du contenu reçu pour un espace (gardé en mémoire jusqu'à sa fusion).
const MAX_SNAPSHOT_BYTES: usize = 1024 * 1024 * 1024;

/// Contenu d'un espace accepté, en cours de réception.
pub(super) struct Incoming {
    /// Nom de l'offre, repli quand `ProfileInfo` n'arrive pas (version sans clone).
    offer_name: String,
    info: Option<ProfileInfoEntry>,
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
                        // Le nom courant du profil prime sur celui de l'espace, figé à sa
                        // création
                        let info = profiles.profile_info(&profile)?;
                        Ok((profile, offer, info.name))
                    })
                    .collect::<AppResult<Vec<_>>>()
            })
            .await?;

        let mut entries = Vec::new();
        for (profile, offer, name) in offers {
            let offer = offer.map(|offer| ProfileOffer {
                name: name.unwrap_or(offer.name),
                ..offer
            });
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

    /// Offre reçue : acceptée d'office, sauf les espaces déjà synchronisés ici (refusés :
    /// l'appartenance arrivera par le document de l'espace).
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

        for (offer, known) in offers.into_iter().zip(known) {
            if known {
                self.reply_offer(offer.space_id, false).await?;
            } else {
                self.accept_offer(offer).await?;
            }
        }
        Ok(())
    }

    /// Accepte le profil proposé : son contenu arrive ensuite, le profil est créé à la fin.
    /// Sans effet si la réception est déjà en cours (offre répétée par une nouvelle connexion).
    async fn accept_offer(&mut self, offer: ProfileOffer) -> Result<()> {
        if self.accepted.contains_key(&offer.space_id) {
            return Ok(());
        }
        let incoming = Incoming {
            offer_name: offer.name,
            info: None,
            space: None,
            notes: Vec::new(),
            bytes: 0,
        };
        self.accepted.insert(offer.space_id, incoming);
        self.reply_offer(offer.space_id, true).await
    }

    /// Répond à une offre.
    async fn reply_offer(&mut self, space: SpaceId, accepted: bool) -> Result<()> {
        let space = *space.as_bytes();
        self.send(&Message::OfferReply { space, accepted }).await
    }

    /// Réponse à notre offre : acceptée, on envoie le nom, l'icône et le contenu de l'espace.
    pub(super) async fn on_offer_reply(&mut self, space: [u8; 32], accepted: bool) -> Result<()> {
        let space = SpaceId::from_bytes(space);
        let Some(profile) = self.offered.remove(&space) else {
            return Ok(());
        };
        if !accepted {
            self.shared.offer_delivered(&self.remote, &profile);
            return Ok(());
        }
        self.send_profile_info(space, &profile).await?;
        self.send_snapshot(space, profile).await
    }

    /// Le nom et l'icône du profil, pour que l'autre appareil en crée le clone. Un profil
    /// illisible ou une icône absente n'empêchent pas la synchronisation : le nom de
    /// l'espace sert alors de repli. Rien ne part à une version qui ne le comprend pas.
    async fn send_profile_info(&mut self, space: SpaceId, profile: &P) -> Result<()> {
        if self.remote_features & FEATURE_PROFILE_INFO == 0 {
            return Ok(());
        }
        let source = profile.clone();
        let info = self
            .shared
            .stores
            .run(move |profiles| {
                let info = profiles.profile_info(&source)?;
                let icon = profiles.profile_icon(&source).unwrap_or_else(|error| {
                    tracing::warn!(%error, "icône du profil illisible : non envoyée");
                    None
                });
                Ok((info, icon))
            })
            .await?;
        let (info, icon) = info;
        let entry = ProfileInfoEntry {
            space: *space.as_bytes(),
            name: info.name,
            icon: icon.map(|icon| icon.bytes),
            is_default: info.is_default,
        };
        self.send(&Message::ProfileInfo(entry)).await
    }

    /// Envoie tous les documents du profil, si l'appareil est encore membre de l'espace : il
    /// l'est depuis le partage du profil, et l'utilisateur a pu décocher entre-temps (rien ne
    /// part alors, et l'appareil n'est pas réadmis).
    async fn send_snapshot(&mut self, space: SpaceId, profile: P) -> Result<()> {
        let remote = self.remote;
        let snapshot = self
            .shared
            .stores
            .with(&profile, move |store| snapshot_docs(store, &remote))
            .await?;
        let Some((docs, upto)) = snapshot else {
            self.shared.offer_delivered(&self.remote, &profile);
            return Ok(());
        };

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

    /// Nom et icône du profil proposé, reçus avant son contenu.
    pub(super) fn on_profile_info(&mut self, entry: ProfileInfoEntry) {
        let Some(incoming) = self.accepted.get_mut(&SpaceId::from_bytes(entry.space)) else {
            return;
        };
        incoming.info = Some(entry);
    }

    /// Un morceau du contenu d'un espace accepté.
    pub(super) fn on_snapshot(&mut self, space: [u8; 32], docs: Vec<DocUpdate>) -> Result<()> {
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
        Ok(())
    }

    /// Fin du contenu : un nouveau profil est créé, clone du profil d'origine (sans mot de
    /// passe maître), et rejoint l'espace ; puis le rattrapage normal démarre.
    pub(super) async fn on_snapshot_end(&mut self, space: [u8; 32], seq: u64) -> Result<()> {
        let space_id = SpaceId::from_bytes(space);
        let Some(incoming) = self.accepted.remove(&space_id) else {
            return Ok(());
        };
        let snapshot = RemoteSnapshot {
            space: incoming.space.ok_or(Error::Malformed)?,
            notes: incoming.notes,
        };
        let shared_default = incoming.info.as_ref().is_some_and(|info| info.is_default);
        let (name, icon) = clone_identity(incoming.info.as_ref(), incoming.offer_name);

        let created = self
            .shared
            .stores
            .run(move |profiles| {
                // Un autre appareil a pu apporter le même espace entre-temps
                if profiles.profile_for(&space_id)?.is_some() {
                    return Ok(None);
                }
                // Le profil par défaut est le même sur toutes les installations : il rejoint
                // l'espace tel quel (nom, icône et mot de passe d'ici inchangés)
                if shared_default && let Some(profile) = profiles.default_profile_to_merge()? {
                    let shown = profiles.profile_info(&profile)?.name;
                    return Ok(Some((profile, shown, true)));
                }
                let profile = profiles.create_profile(name.as_deref(), icon.as_ref())?;
                let shown = profiles.profile_info(&profile)?.name;
                Ok(Some((profile, shown, false)))
            })
            .await?;
        let Some((profile, shown, merged)) = created else {
            return Ok(());
        };
        let applied = self
            .shared
            .stores
            .with(&profile, move |store| {
                store.adopt_space(&space_id, &snapshot)
            })
            .await?;
        self.shared.publish(SyncEvent::NotesSynced {
            profile: profile.clone(),
            notes: applied.notes,
            folders: true,
            from: self.remote_name.clone(),
        });
        if let Some(device_name) = self.remote_name.clone() {
            self.shared.publish(SyncEvent::ProfileReceived {
                profile,
                name: shown.unwrap_or_default(),
                device: self.remote,
                device_name,
                merged,
            });
        }

        self.shared.state().joining.insert(space_id);
        self.send(&Message::Ack { space, seq }).await?;
        self.announce().await?;
        crate::engine::request_refresh(&self.shared);
        Ok(())
    }
}

/// Le nom et l'icône du clone : ceux du profil d'origine quand `ProfileInfo` est arrivé (un
/// nom absent laisse le nom par défaut de cette installation), sinon le nom de l'offre. Une
/// icône invalide est ignorée.
fn clone_identity(
    info: Option<&ProfileInfoEntry>,
    offer_name: String,
) -> (Option<String>, Option<ProfileIcon>) {
    let Some(info) = info else {
        let name = Some(offer_name).filter(|name| !name.is_empty());
        return (name, None);
    };
    let icon = info.icon.as_ref().and_then(|bytes| {
        validate_icon(bytes).map(|format| ProfileIcon {
            format,
            bytes: bytes.clone(),
        })
    });
    (info.clean_name(), icon)
}

/// Tous les documents du profil en entier et le dernier `seq` écrit ; `None` si l'appareil
/// n'est pas (ou plus) membre non révoqué de l'espace.
fn snapshot_docs(
    store: &dyn SyncStore,
    remote: &DeviceKey,
) -> AppResult<Option<(Vec<DocUpdate>, u64)>> {
    if !is_member(&store.devices()?, remote) {
        return Ok(None);
    }
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
    Ok(Some((docs, upto)))
}
