//! Partage des collections : « Synchroniser avec mes appareils », réponse à une proposition,
//! arrêt du partage, changements sur le disque.
//!
//! Une collection ne se partage qu'avec les appareils qui partagent déjà le profil qui la
//! porte : les propositions voyagent dans la session de ce profil.

use std::path::PathBuf;
use std::sync::Arc;

use app::Refusal;
use app::ports::collection_sync::ShareRecord;
use domain::sync::DeviceKey;
use domain::uuid::Uuid;

use super::{Command, IncomingCollection, Shared};
use crate::error::{Error, Result};
use crate::files::{Ports, ShareCtx};
use crate::stores::{ProfileKey, is_member};

/// Réponse de l'utilisateur à une collection proposée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollectionAnswer {
    /// Recevoir la collection dans `destination` (dossier créé au besoin). Avec `merge`, un
    /// dossier qui contient déjà des fichiers est accepté : les fichiers identiques sont
    /// gardés, les absents copiés, et deux versions différentes d'un même fichier sont
    /// gardées toutes les deux (copie de conflit). Sans `merge`, un dossier non vide est
    /// refusé.
    Accept {
        /// Dossier de destination, chemin absolu.
        destination: PathBuf,
        /// Fusionner avec le contenu d'un dossier existant.
        merge: bool,
    },
    /// Refuser cette proposition (les suivantes ne sont pas bloquées).
    Decline,
}

/// Propose la collection `root` de `profile` à `devices`, tous membres du profil.
pub(super) async fn share<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    profile: P,
    root: String,
    devices: Vec<DeviceKey>,
) -> Result<ShareRecord> {
    let id = random_id()?;
    let record = shared
        .stores
        .with(&profile, move |store| {
            let members = store.devices()?;
            if devices.is_empty() || !devices.iter().all(|key| is_member(&members, key)) {
                return Err(Refusal::UnknownShare.into());
            }
            store.create_share(&root, &id, &devices)
        })
        .await?;

    for peer in &record.peers {
        shared.command(&peer.device, Command::CollectionsChanged);
    }
    shared.collection_changed(&profile, record.sync_id);
    Ok(record)
}

/// Arrête de partager la collection : les fichiers restent des deux côtés, le lien est coupé.
pub(super) async fn stop<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    profile: P,
    share: Uuid,
) -> Result<()> {
    let record = shared
        .stores
        .with(&profile, move |store| {
            let record = store.share(&share)?;
            store.stop_share(&share)?;
            Ok(record)
        })
        .await?;
    let Some(record) = record else {
        return Err(Error::UnknownOffer);
    };

    for peer in &record.peers {
        shared.command(&peer.device, Command::StopShare(share));
    }
    shared.state().collections.remove(&share);
    shared.collection_changed(&profile, share);
    Ok(())
}

/// Répond à une collection proposée ([`SyncEvent::CollectionOfferReceived`]) : rien n'est écrit
/// sur le disque avant l'acceptation.
///
/// [`SyncEvent::CollectionOfferReceived`]: app::ports::sync::SyncEvent::CollectionOfferReceived
pub(super) async fn answer<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    device: DeviceKey,
    share: Uuid,
    answer: CollectionAnswer,
) -> Result<()> {
    let incoming = shared
        .pending_collections()
        .into_iter()
        .find(|incoming| incoming.device == device && incoming.offer.share == share)
        .ok_or(Error::UnknownOffer)?;
    let accepted = match answer {
        CollectionAnswer::Decline => false,
        CollectionAnswer::Accept { destination, merge } => {
            accept(shared, &incoming, destination, merge).await?;
            true
        }
    };

    shared.forget_collection(&device, &share);
    shared.command(&device, Command::CollectionAnswer { share, accepted });
    shared.collection_changed(&incoming.profile, share);
    Ok(())
}

/// Prépare le dossier et enregistre la collection reçue, sans rien écrire dedans.
async fn accept<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    incoming: &IncomingCollection<P>,
    destination: PathBuf,
    merge: bool,
) -> Result<()> {
    if !destination.is_absolute() {
        return Err(Error::App(Refusal::OutsideCollection.into()));
    }
    let (device, share) = (incoming.device, incoming.offer.share);
    let name = incoming.offer.name.clone();
    shared
        .with_ports(&incoming.profile, move |ports| {
            let occupied = ports.disk.is_directory(&destination)
                && !ports.disk.is_empty_directory(&destination);
            if occupied && !merge {
                return Err(Refusal::DestinationNotEmpty.into());
            }
            let root = ports.disk.prepare_directory(&destination)?;
            let root = root.to_string_lossy();
            ports
                .store
                .accept_share(&share, &root, &name, &device)
                .map(drop)
        })
        .await
}

/// Des fichiers du partage `share` ont changé sur le disque (ou au démarrage) : le résumé du
/// dossier est recalculé, et les appareils connectés reçoivent un manifeste à jour.
pub(super) async fn changed<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    profile: P,
    share: Uuid,
) -> Result<()> {
    let key = shared.identity.key();
    let summary = shared
        .with_ports(&profile, move |ports| summary_of(ports, share, key))
        .await?;
    if let Some(summary) = summary
        && shared.set_summary(share, summary)
    {
        shared.collection_changed(&profile, share);
    }
    shared.request_share_push(share);
    Ok(())
}

/// Le résumé du dossier d'un partage, `None` si le partage n'existe plus.
fn summary_of(
    ports: &Ports<'_>,
    share: Uuid,
    me: DeviceKey,
) -> app::AppResult<Option<crate::files::LocalSummary>> {
    let Some(record) = ports.store.share(&share)? else {
        return Ok(None);
    };
    let root = PathBuf::from(&record.root);
    let remote = record.peers.first().map_or(me, |peer| peer.device);
    let ctx = ShareCtx {
        id: share,
        root: &root,
        remote,
        role: app::collection_sync::Role::between(&me, &remote),
        my_name: "",
    };
    let now = crate::files::now_nanos();
    Ok(Some(match crate::files::snapshot(ports, &ctx, now)? {
        Some(snapshot) => snapshot.summary,
        None => crate::files::LocalSummary {
            root_missing: true,
            ..crate::files::LocalSummary::default()
        },
    }))
}

fn random_id() -> Result<Uuid> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| Error::Stopped)?;
    Ok(Uuid::from_random(bytes))
}
