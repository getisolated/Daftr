//! Partage d'un profil avec un appareil et retraits : « Synchroniser avec… », « Arrêter de
//! synchroniser ce profil avec… », « Retirer l'appareil ».
//!
//! Le profil proposé est accepté d'office par l'appareil qui le reçoit (voir
//! `session::offers`) : il n'y a pas de réponse de l'utilisateur à donner.

use std::sync::Arc;

use app::ports::sync::SyncEvent;
use app::ports::trust::{TrustOrigin, TrustedDevice};
use domain::sync::{DeviceKey, DeviceKind, DeviceName};

use super::links::{self, fallback_name};
use super::{Command, Shared};
use crate::error::{Error, Result};
use crate::stores::{self, ProfileKey, in_store};

/// Partage `profile` avec `device` : l'appareil devient membre de l'espace (écriture locale
/// persistante), puis le profil lui est proposé tout de suite s'il est connecté, sinon dès
/// qu'il apparaît (une tentative immédiate est faite vers ses adresses connues).
pub(super) async fn offer<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    profile: P,
    device: DeviceKey,
) -> Result<()> {
    let name = trusted_name(shared, device)
        .await?
        .ok_or(Error::Untrusted)?;
    let now = shared.clock.timestamp();
    let changed = shared
        .stores
        .with(&profile, move |store| {
            if store.space_id()?.is_none() {
                return Ok(None);
            }
            store.add_device(&device, &name, &now).map(Some)
        })
        .await?
        .ok_or(Error::UnknownOffer)?;
    {
        let mut state = shared.state();
        let pending = state.outgoing_offers.entry(device).or_default();
        if !pending.contains(&profile) {
            pending.push(profile.clone());
        }
    }
    if changed {
        // Membre (de nouveau) : l'espace est annoncé à l'appareil, qui le connaît peut-être déjà
        shared.command(&device, Command::Announce);
        shared.request_push(profile, None);
    }
    if shared.command(&device, Command::SendOffers) {
        return Ok(());
    }
    let addrs = links::known_addrs(shared, device).await?;
    let task_shared = Arc::clone(shared);
    shared.runtime.spawn(async move {
        links::connect_or_retry(&task_shared, device, &addrs).await;
    });
    Ok(())
}

/// Le nom d'un appareil de confiance non révoqué, `None` s'il ne l'est pas.
async fn trusted_name<P: ProfileKey>(
    shared: &Shared<P>,
    key: DeviceKey,
) -> Result<Option<DeviceName>> {
    shared
        .with_trust(move |trust| {
            Ok(trust
                .devices()?
                .into_iter()
                .find(|device| device.key == key && device.is_trusted())
                .map(|device| device.name))
        })
        .await
}

/// « Arrêter de synchroniser ce profil avec <appareil> » : révocation dans l'espace. Ses
/// collections partagées ne changent pas : elles ne dépendent pas du profil.
pub(super) async fn stop_sharing<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    profile: P,
    device: DeviceKey,
) -> Result<()> {
    let now = shared.clock.timestamp();
    let changed = shared
        .stores
        .with(&profile, move |store| store.revoke_device(&device, &now))
        .await?;
    shared.offer_delivered(&device, &profile);
    shared.command(&device, Command::Announce);
    if changed {
        shared.request_push(profile, None);
    }
    Ok(())
}

/// « Retirer l'appareil » : révoqué dans la liste de confiance (définitif) et dans tous les
/// espaces dont il est membre ; ses partages de collections et ses propositions en attente
/// disparaissent ; sa session est fermée, après lui avoir dit qu'il est retiré (`Removed`).
/// Les notes et fichiers locaux ne sont jamais touchés.
pub(super) async fn remove_device<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    device: DeviceKey,
) -> Result<()> {
    // L'ordre part avant la fermeture : le canal d'ordres livre ce qu'il contient avant de
    // se terminer, la session envoie `Removed` puis ferme
    shared.command(&device, Command::Removed);
    unlink(shared, device).await
}

/// `device` nous a retirés de ses appareils (`Removed`, reçu sur un canal authentifié) : même
/// effet qu'un retrait volontaire, sans lui répondre. Rend `false`, sans rien changer, si
/// `device` n'est pas de confiance (inconnu, ou déjà retiré : pas de rebond ni de doublon).
pub(crate) async fn apply_removed<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    device: DeviceKey,
) -> Result<bool> {
    let Some(name) = trusted_name(shared, device).await? else {
        return Ok(false);
    };
    unlink(shared, device).await?;
    shared.publish(SyncEvent::DeviceRemoved { device, name });
    Ok(true)
}

async fn unlink<P: ProfileKey>(shared: &Arc<Shared<P>>, device: DeviceKey) -> Result<()> {
    let now = shared.clock.timestamp();
    revoke_trust(shared, device, now.clone()).await?;
    let (changed, dropped) = shared
        .stores
        .run(move |profiles| {
            let mut changed = Vec::new();
            let mut dropped = Vec::new();
            for profile in profiles.share_profiles()? {
                let shares = in_store(profiles, &profile, |store| store.shares())?;
                for share in shares.iter().filter(|share| share.peer(&device).is_some()) {
                    dropped.push((profile.clone(), share.sync_id));
                }
                in_store(profiles, &profile, |store| store.drop_device(&device))?;
            }
            for profile in stores::enabled_profiles(profiles)? {
                if in_store(profiles, &profile, |store| {
                    store.revoke_device(&device, &now)
                })? {
                    changed.push(profile);
                }
            }
            Ok((changed, dropped))
        })
        .await?;
    shared
        .with_inbox(move |inbox| inbox.remove_device(&device))
        .await?;

    {
        let mut state = shared.state();
        state.outgoing_offers.remove(&device);
        if let Some(retry) = state.retries.remove(&device).and_then(|retry| retry.task) {
            retry.abort();
        }
        for (_, share) in &dropped {
            state.collections.remove(share);
        }
    }
    shared.close_session(&device);
    for (profile, share) in &dropped {
        shared.collection_changed(profile, *share);
    }
    shared.publish(SyncEvent::CollectionPendingChanged);
    for profile in changed {
        shared.request_push(profile, None);
    }
    super::runtime::request_refresh(shared);
    shared.command_all(&Command::Announce);
    shared.publish_status();
    Ok(())
}

async fn revoke_trust<P: ProfileKey>(
    shared: &Shared<P>,
    device: DeviceKey,
    now: String,
) -> Result<()> {
    shared
        .with_trust(move |trust| {
            let known = trust
                .devices()?
                .into_iter()
                .find(|known| known.key == device);
            let Some(name) = known
                .as_ref()
                .map(|known| known.name.clone())
                .or_else(|| fallback_name(&device))
            else {
                return Ok(());
            };
            let (origin, added_at, kind) = known.map_or(
                (TrustOrigin::Paired, now.clone(), DeviceKind::Unknown),
                |known| (known.origin, known.added_at, known.kind),
            );
            trust.put_device(&TrustedDevice {
                key: device,
                name,
                kind,
                origin,
                added_at,
                revoked_at: Some(now),
            })
        })
        .await
}
