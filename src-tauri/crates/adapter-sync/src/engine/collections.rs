//! Partage des collections : « Synchroniser avec mes appareils », activation ou refus d'une
//! collection reçue, arrêt du partage, changements sur le disque.
//!
//! Un partage ne dépend que de la confiance de l'appareil : l'appareil n'a pas besoin de
//! partager le profil qui porte la collection. Une collection reçue reste une entrée « à
//! configurer » (gardée hors des bases de profil) jusqu'à ce que l'utilisateur choisisse le
//! profil et le dossier qui la reçoivent.

use std::path::PathBuf;
use std::sync::Arc;

use app::Refusal;
use app::ports::collection_sync::{PendingCollection, ShareRecord};
use app::ports::sync::SyncEvent;
use domain::sync::DeviceKey;
use domain::uuid::Uuid;

use super::{Command, Shared, links};
use crate::error::{Error, Result};
use crate::files::{Ports, ShareCtx};
use crate::stores::ProfileKey;

/// Propose la collection `root` de `profile` à `devices`, tous de confiance.
pub(super) async fn share<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    profile: P,
    root: String,
    devices: Vec<DeviceKey>,
) -> Result<ShareRecord> {
    ensure_trusted(shared, &devices).await?;
    let id = random_id()?;
    let record = shared
        .stores
        .with(&profile, move |store| {
            store.create_share(&root, &id, &devices)
        })
        .await?;

    for peer in &record.peers {
        shared.command(&peer.device, Command::CollectionsChanged);
    }
    shared.collection_changed(&profile, record.sync_id);
    Ok(record)
}

/// Partage `root` avec `device` seul (`on`), ou l'en retire. Rend le partage tel qu'il est
/// ensuite, `None` s'il n'existe plus.
pub(super) async fn set_device<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    profile: P,
    root: String,
    device: DeviceKey,
    on: bool,
) -> Result<Option<ShareRecord>> {
    if on {
        return share(shared, profile, root, vec![device]).await.map(Some);
    }
    let (left, id) = shared
        .stores
        .with(&profile, move |store| {
            let Some(record) = store.share_of(&root)? else {
                return Ok((None, None));
            };
            if record.peer(&device).is_none() {
                return Ok((Some(record), None));
            }
            store.set_peer_state(&record.sync_id, &device, None)?;
            let remaining = store.share(&record.sync_id)?;
            if remaining
                .as_ref()
                .is_some_and(|share| share.peers.is_empty())
            {
                store.stop_share(&record.sync_id)?;
                return Ok((None, Some(record.sync_id)));
            }
            Ok((remaining, Some(record.sync_id)))
        })
        .await?;
    if let Some(share) = id {
        // L'appareil absent est prévenu à sa reconnexion (il fait vérifier ses partages)
        shared.command(&device, Command::StopShare(share));
        if left.is_none() {
            shared.state().collections.remove(&share);
        }
        shared.collection_changed(&profile, share);
    }
    Ok(left)
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

/// Les entrées « à configurer », triées.
pub(super) async fn pending<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
) -> Result<Vec<PendingCollection>> {
    let mut entries = shared.with_inbox(|inbox| inbox.pending()).await?;
    entries.retain(|entry| !entry.ignored);
    entries.sort_by_key(|entry| (entry.device, entry.offer.share));
    Ok(entries)
}

/// Active une collection reçue dans le profil choisi (actif ou non) : rien n'est écrit sur le
/// disque avant cet appel.
pub(super) async fn activate<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    device: DeviceKey,
    share: Uuid,
    profile: P,
    destination: PathBuf,
    merge: bool,
) -> Result<()> {
    let entry = pending(shared)
        .await?
        .into_iter()
        .find(|entry| entry.device == device && entry.offer.share == share)
        .ok_or(Error::UnknownOffer)?;
    if !destination.is_absolute() {
        return Err(Error::App(Refusal::OutsideCollection.into()));
    }
    let name = entry.offer.name;
    shared
        .with_ports(&profile, move |ports| {
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
        .await?;

    shared
        .with_inbox(move |inbox| inbox.remove(&device, &share))
        .await?;
    let accepted = Command::CollectionAnswer {
        share,
        accepted: true,
    };
    shared.command(&device, accepted);
    shared.collection_changed(&profile, share);
    shared.publish(SyncEvent::CollectionPendingChanged);
    Ok(())
}

/// Ignore une collection reçue : l'entrée est retirée de la liste, et le refus part à
/// l'appareil, tout de suite s'il est connecté, sinon à sa prochaine proposition.
pub(super) async fn ignore<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    device: DeviceKey,
    share: Uuid,
) -> Result<()> {
    let found = shared
        .with_inbox(move |inbox| {
            let entry = inbox
                .pending()?
                .into_iter()
                .find(|entry| entry.device == device && entry.offer.share == share);
            let Some(mut entry) = entry else {
                return Ok(false);
            };
            entry.ignored = true;
            inbox.put(&entry)?;
            Ok(true)
        })
        .await?;
    if !found {
        return Err(Error::UnknownOffer);
    }
    let declined = Command::CollectionAnswer {
        share,
        accepted: false,
    };
    if shared.command(&device, declined) {
        shared
            .with_inbox(move |inbox| inbox.remove(&device, &share))
            .await?;
    }
    shared.publish(SyncEvent::CollectionPendingChanged);
    Ok(())
}

/// Les appareils sont tous de confiance (et il y en a au moins un).
async fn ensure_trusted<P: ProfileKey>(shared: &Shared<P>, devices: &[DeviceKey]) -> Result<()> {
    let trusted = links::trusted_devices(shared).await?;
    if devices.is_empty() || !devices.iter().all(|key| trusted.contains(key)) {
        return Err(Error::Untrusted);
    }
    Ok(())
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
