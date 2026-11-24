//! Session `Sync` avec un appareil : une seule connexion par paire d'appareils, qui porte
//! tous leurs espaces communs. Chaque côté annonce dans `Hello` les seuls espaces où l'autre
//! est membre ; les espaces échangés sont l'intersection. La session traite les messages un
//! par un (l'ordre de TCP garantit qu'un `Done` arrive après les updates qu'il clôt).

mod catchup;
mod collections;
mod offers;
mod transfer;

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;

use app::ports::sync::{Applied, SyncEvent};
use app::ports::trust::{TrustOrigin, TrustedDevice};
use domain::sync::{DeviceKey, DeviceKind, DeviceName, SpaceId};
use domain::uuid::Uuid;
use tokio::net::TcpStream;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;

use crate::engine::{Command, Shared};
use crate::error::{Error, Result};
use crate::noise::{Channel, ChannelReader, ChannelWriter};
use crate::pairing::LinkedDevice;
use crate::stores::{self, ProfileKey};
use crate::wire::{
    self, FEATURE_COLLECTIONS, FEATURE_PROFILE_INFO, Hello, MAX_SPACES, Message, PROTOCOL_VERSION,
};

use catchup::Round;
use collections::ShareSession;
use offers::Incoming;
use transfer::Chunk;

/// Appairage à conclure au premier `Hello` : confiance, événement, réponse à l'appelant.
pub(crate) struct Linking {
    /// Rend l'appareil lié à l'appelant de l'appairage (côté qui compose).
    pub reply: Option<oneshot::Sender<LinkedDevice>>,
}

/// Ouverture d'une session.
pub(crate) struct Opening {
    /// L'appareil qui a composé.
    pub initiator: DeviceKey,
    /// L'adresse composée (côté qui compose) : la dernière adresse connue de l'appareil.
    pub addr: Option<SocketAddr>,
    /// Appairage à conclure.
    pub linking: Option<Linking>,
}

/// Démarre la session sur un canal établi ; `false` si une autre connexion avec le même
/// appareil l'emporte (le canal est alors fermé).
pub(crate) fn spawn<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    channel: Channel<TcpStream>,
    opening: Opening,
) -> bool {
    let (commands, receiver) = mpsc::unbounded_channel();
    let remote = channel.remote;
    let Some(id) = shared.register_session(remote, opening.initiator, commands) else {
        return false;
    };
    let (reader, writer) = channel.split();
    let session = Session {
        shared: Arc::clone(shared),
        id,
        remote,
        addr: opening.addr,
        linking: opening.linking,
        writer,
        remote_name: None,
        local: HashMap::new(),
        remote_spaces: HashSet::new(),
        rounds: HashMap::new(),
        offered: HashMap::new(),
        snapshots_sent: HashSet::new(),
        accepted: HashMap::new(),
        remote_features: 0,
        shares: HashMap::new(),
        offered_collections: HashSet::new(),
        pending_sent: false,
    };
    shared.runtime.spawn(session.run(reader, receiver));
    true
}

/// Fin d'une session.
enum End {
    /// Remplacée par une autre connexion, ou fermée par le moteur.
    Closed,
    /// Échec ou fermeture distante : on retentera.
    Failed(Error),
}

struct Session<P: ProfileKey> {
    shared: Arc<Shared<P>>,
    id: u64,
    remote: DeviceKey,
    addr: Option<SocketAddr>,
    linking: Option<Linking>,
    writer: ChannelWriter<TcpStream>,
    remote_name: Option<DeviceName>,
    /// Espaces que nous annonçons, avec leur profil local.
    local: HashMap<SpaceId, P>,
    /// Espaces annoncés par l'appareil distant.
    remote_spaces: HashSet<SpaceId>,
    /// Nos rattrapages en cours, par espace.
    rounds: HashMap<SpaceId, Round>,
    /// Nos offres en attente de réponse.
    offered: HashMap<SpaceId, P>,
    /// Espaces dont nous avons envoyé le contenu, en attente d'acquittement.
    snapshots_sent: HashSet<SpaceId>,
    /// Offres acceptées dont nous recevons le contenu.
    accepted: HashMap<SpaceId, Incoming>,
    /// Fonctionnalités annoncées par l'appareil distant.
    remote_features: u8,
    /// Collections partagées actives avec l'appareil distant.
    shares: HashMap<Uuid, ShareSession<P>>,
    /// Collections proposées dans cette session, en attente de réponse.
    offered_collections: HashSet<Uuid>,
    /// La liste des collections reçues en attente a été envoyée dans cette session.
    pending_sent: bool,
}

type Inbound = UnboundedReceiver<Result<Message>>;

impl<P: ProfileKey> Session<P> {
    async fn run(
        mut self,
        reader: ChannelReader<TcpStream>,
        mut commands: UnboundedReceiver<Command<P>>,
    ) {
        let (sender, mut inbound) = mpsc::unbounded_channel();
        let reading = self.shared.runtime.spawn(read_loop(reader, sender));
        let end = match self.serve(&mut inbound, &mut commands).await {
            Ok(()) => End::Closed,
            Err(error) => End::Failed(error),
        };
        reading.abort();
        self.finish(end);
    }

    async fn serve(
        &mut self,
        inbound: &mut Inbound,
        commands: &mut UnboundedReceiver<Command<P>>,
    ) -> Result<()> {
        self.announce().await?;
        self.shared.publish_status();
        loop {
            tokio::select! {
                message = inbound.recv() => match message {
                    Some(message) => self.handle(message?).await?,
                    None => return Err(Error::Closed),
                },
                command = commands.recv() => match command {
                    Some(command) => self.execute(command).await?,
                    None => return Ok(()),
                },
            }
        }
    }

    fn finish(self, end: End) {
        let current = self.shared.unregister_session(&self.remote, self.id);
        self.shared.collections_disconnected(&self.remote);
        self.shared.publish_status();
        if let End::Failed(error) = end {
            tracing::debug!(remote = %self.remote, %error, "session terminée");
            if current && self.remote_name.is_some() {
                crate::engine::schedule_retry(&self.shared, self.remote);
            }
        }
    }

    async fn handle(&mut self, message: Message) -> Result<()> {
        if self.remote_name.is_none() && !matches!(message, Message::Hello(_)) {
            return Err(Error::Unexpected);
        }
        match message {
            Message::Hello(hello) => self.on_hello(hello).await,
            Message::Changed { space, docs } => self.on_changed(space, docs).await,
            Message::Diffs { space, docs, last } => self.on_diffs(space, docs, last).await,
            Message::Updates { space, docs } => self.on_updates(space, docs).await,
            Message::Done { space, seq } => self.on_done(space, seq).await,
            Message::Ack { space, seq } => self.on_ack(space, seq).await,
            Message::Offer(entries) => self.on_offer(entries).await,
            Message::OfferReply { space, accepted } => self.on_offer_reply(space, accepted).await,
            Message::Snapshot { space, docs, .. } => self.on_snapshot(space, docs),
            Message::ProfileInfo(entry) => {
                self.on_profile_info(entry);
                Ok(())
            }
            Message::CollectionPending { shares } => self.on_collection_pending(shares).await,
            Message::SnapshotEnd { space, seq } => self.on_snapshot_end(space, seq).await,
            Message::CollectionOffer(entries) => self.on_collection_offer(entries).await,
            Message::CollectionReply { share, accepted } => {
                self.on_collection_reply(share, accepted).await
            }
            Message::Manifest {
                share,
                entries,
                last,
            } => self.on_manifest(share, entries, last).await,
            Message::FileRequest { share, path, hash } => {
                self.on_file_request(share, path, hash).await
            }
            Message::FileChunk {
                share,
                path,
                hash,
                total,
                offset,
                data,
            } => {
                let chunk = Chunk {
                    share,
                    path,
                    hash,
                    total,
                    offset,
                    data,
                };
                self.on_file_chunk(chunk).await
            }
            Message::FileUnavailable { share, path, hash } => {
                self.on_file_unavailable(share, path, hash).await
            }
            Message::FileAck { share, path, hash } => self.on_file_ack(share, path, hash).await,
            Message::ShareStop { share } => self.on_share_stop(share).await,
            Message::Open(_) | Message::Spake(_) | Message::Confirm(_) | Message::Refused(_) => {
                Err(Error::Unexpected)
            }
        }
    }

    async fn execute(&mut self, command: Command<P>) -> Result<()> {
        match command {
            Command::Announce => self.announce().await,
            Command::Push(profile) => match self.common_space_of(&profile) {
                Some(space) => self.start_round(space).await,
                None => Ok(()),
            },
            Command::SendOffers => self.send_offers().await,
            Command::CollectionsChanged => self.sync_collections().await,
            Command::ShareChanged(share) => self.share_changed(share).await,
            Command::CollectionAnswer { share, accepted } => {
                self.collection_answer(share, accepted).await
            }
            Command::StopShare(share) => self.stop_share(share).await,
        }
    }

    async fn send(&mut self, message: &Message) -> Result<()> {
        self.writer.send(message).await
    }

    /// Annonce (ou réannonce) les espaces partagés avec l'appareil distant.
    async fn announce(&mut self) -> Result<()> {
        let remote = self.remote;
        let spaces = self
            .shared
            .stores
            .run(move |profiles| stores::shared_spaces(profiles, &remote))
            .await?;
        let hello = Hello {
            protocol: PROTOCOL_VERSION,
            name: self.shared.state().device_name.as_str().to_owned(),
            spaces: spaces.keys().map(|space| *space.as_bytes()).collect(),
            kind: self.shared.config.device_kind,
            features: FEATURE_COLLECTIONS | FEATURE_PROFILE_INFO,
        };
        self.send(&Message::Hello(hello)).await?;

        let before = self.common();
        self.local = spaces;
        self.spaces_changed(&before).await?;
        self.sync_collections().await
    }

    async fn on_hello(&mut self, hello: Hello) -> Result<()> {
        if hello.protocol != PROTOCOL_VERSION {
            return Err(Error::Protocol(hello.protocol));
        }
        if hello.spaces.len() > MAX_SPACES {
            return Err(Error::Malformed);
        }
        let first = self.remote_name.is_none();
        self.remote_name = Some(wire::device_name(&hello.name)?);
        self.remote_features = hello.features;
        let before = self.common();
        self.remote_spaces = hello.spaces.into_iter().map(SpaceId::from_bytes).collect();

        if first {
            self.link(hello.kind).await?;
        }
        self.remember_kind(hello.kind).await;
        self.spaces_changed(&before).await?;
        if first {
            self.send_offers().await?;
        }
        self.sync_collections().await
    }

    /// Conclut un appairage : l'appareil devient de confiance (origine « appairé »).
    async fn link(&mut self, kind: DeviceKind) -> Result<()> {
        let (Some(linking), Some(name)) = (self.linking.take(), self.remote_name.clone()) else {
            return Ok(());
        };
        let device = TrustedDevice {
            key: self.remote,
            name: name.clone(),
            kind,
            origin: TrustOrigin::Paired,
            added_at: self.shared.clock.timestamp(),
            revoked_at: None,
        };
        self.shared
            .with_trust(move |trust| trust.put_device(&device))
            .await?;
        self.shared.publish(SyncEvent::DeviceLinked {
            device: self.remote,
            name: name.clone(),
        });
        // Côté qui compose : l'appelant a pu abandonner, l'appairage reste conclu. Côté qui
        // affichait le code, le mode appairage est déjà fini.
        if let Some(reply) = linking.reply {
            let _ = reply.send(LinkedDevice {
                key: self.remote,
                name,
            });
        }
        // Un appareil de confiance de plus : l'écoute reste ouverte pour ses offres, des
        // deux côtés
        crate::engine::request_refresh(&self.shared);
        Ok(())
    }

    /// Garde le type annoncé avec l'appareil de confiance. Un échec d'écriture n'interrompt
    /// pas la session : le type n'est qu'une icône, il sera réécrit au prochain `Hello`.
    async fn remember_kind(&self, kind: DeviceKind) {
        let remote = self.remote;
        let saved = self
            .shared
            .with_trust(move |trust| trust.set_device_kind(&remote, kind))
            .await;
        if let Err(error) = saved {
            tracing::warn!(remote = %self.remote, %error, "type d'appareil non enregistré");
        }
    }

    /// Les espaces échangés : annoncés des deux côtés.
    fn common(&self) -> HashSet<SpaceId> {
        self.local
            .keys()
            .filter(|space| self.remote_spaces.contains(space))
            .copied()
            .collect()
    }

    fn is_common(&self, space: &SpaceId) -> bool {
        self.local.contains_key(space) && self.remote_spaces.contains(space)
    }

    /// Le profil local d'un espace échangé.
    fn common_profile(&self, space: &SpaceId) -> Option<P> {
        self.is_common(space)
            .then(|| self.local.get(space).cloned())
            .flatten()
    }

    fn common_space_of(&self, profile: &P) -> Option<SpaceId> {
        self.local
            .iter()
            .find(|(space, local)| *local == profile && self.remote_spaces.contains(space))
            .map(|(space, _)| *space)
    }

    /// Rattrapage des espaces devenus communs ; abandon de ceux qui ne le sont plus.
    async fn spaces_changed(&mut self, before: &HashSet<SpaceId>) -> Result<()> {
        let now = self.common();
        self.rounds.retain(|space, _| now.contains(space));
        self.shared.set_syncing(self.id, !self.rounds.is_empty());
        for space in now.difference(before).copied().collect::<Vec<_>>() {
            self.start_round(space).await?;
        }
        Ok(())
    }

    /// Après l'application de modifications distantes : événements, propagation vers les
    /// autres appareils, et confiance transitive si l'espace a changé.
    fn applied(&self, profile: &P, applied: Applied) {
        if !applied.rejected.is_empty() {
            let rejected = applied.rejected.len();
            tracing::warn!(remote = %self.remote, rejected, "documents refusés");
        }
        if applied.notes.is_empty() && !applied.space {
            return;
        }
        self.shared.publish(SyncEvent::NotesSynced {
            profile: profile.clone(),
            notes: applied.notes,
            folders: applied.space,
            from: self.remote_name.clone(),
        });
        self.shared.request_push(profile.clone(), Some(self.id));
        if applied.space {
            crate::engine::request_refresh(&self.shared);
        }
    }
}

async fn read_loop(mut reader: ChannelReader<TcpStream>, sender: UnboundedSender<Result<Message>>) {
    loop {
        let message = reader.recv().await;
        // Un message d'une version plus récente est ignoré : la session continue
        if matches!(message, Err(Error::UnknownMessage)) {
            tracing::debug!("message de protocole inconnu ignoré");
            continue;
        }
        let failed = message.is_err();
        if sender.send(message).is_err() || failed {
            return;
        }
    }
}
