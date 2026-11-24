//! Offres de profils et retraits : « Synchroniser avec… », réponse à une offre, « Arrêter de
//! synchroniser ce profil avec… », « Retirer l'appareil ».

use std::sync::Arc;

use app::SyncError;
use app::ports::sync::ProfileOffer;
use app::ports::trust::{TrustOrigin, TrustedDevice};
use domain::sync::{DeviceKey, DeviceKind, SpaceId};

use super::links::{self, fallback_name, is_trusted};
use super::{Command, OfferChoice, Shared, runtime};
use crate::error::{Error, Result};
use crate::stores::{self, ProfileKey, in_store};

/// Propose `profile` à `device` : tout de suite s'il est connecté, sinon dès qu'il apparaît
/// (une tentative immédiate est faite vers ses adresses connues).
pub(super) async fn offer<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    profile: P,
    device: DeviceKey,
) -> Result<()> {
    if !is_trusted(shared, device).await? {
        return Err(Error::Untrusted);
    }
    {
        let mut state = shared.state();
        let pending = state.outgoing_offers.entry(device).or_default();
        if !pending.contains(&profile) {
            pending.push(profile);
        }
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

/// Répond à une offre reçue. « Nouveau profil » est refusé si l'espace est déjà synchronisé
/// ici (un espace par appareil) ; « Fusionner » seulement avec un profil non synchronisé.
pub(super) async fn answer<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    device: DeviceKey,
    space: SpaceId,
    choice: OfferChoice<P>,
) -> Result<()> {
    let offer = shared
        .state()
        .incoming_offers
        .get(&(device, space))
        .cloned()
        .ok_or(Error::UnknownOffer)?;
    let Some(profile) = target_profile(shared, &offer, choice).await? else {
        // Hors connexion, l'appareil reproposera l'offre à la prochaine
        shared.forget_offer(&device, &space);
        shared.command(&device, Command::DeclineOffer { space });
        return runtime::refresh(shared).await;
    };

    shared.forget_offer(&device, &space);
    shared
        .state()
        .accepted_offers
        .insert((device, space), profile.clone());
    if shared.command(&device, Command::AcceptOffer { space, profile }) {
        return Ok(());
    }
    // Connexion perdue entre-temps : l'offre, reproposée à la reconnexion, sera acceptée
    // d'office avec ce choix
    let addrs = links::known_addrs(shared, device).await?;
    let task_shared = Arc::clone(shared);
    shared.runtime.spawn(async move {
        links::connect_or_retry(&task_shared, device, &addrs).await;
    });
    Ok(())
}

/// Le profil local qui rejoindra l'espace proposé, `None` si l'offre est ignorée.
async fn target_profile<P: ProfileKey>(
    shared: &Shared<P>,
    offer: &ProfileOffer,
    choice: OfferChoice<P>,
) -> Result<Option<P>> {
    let already_synced = || Error::App(SyncError::AlreadySynced.into());
    match choice {
        OfferChoice::Ignore => Ok(None),
        OfferChoice::NewProfile => {
            let (space, name) = (offer.space_id, offer.name.clone());
            let existing = shared
                .stores
                .run(move |profiles| profiles.profile_for(&space))
                .await?;
            if existing.is_some() {
                return Err(already_synced());
            }
            let profile = shared
                .stores
                .run(move |profiles| profiles.create_profile(&name))
                .await?;
            Ok(Some(profile))
        }
        OfferChoice::Merge(profile) => {
            let synced = shared
                .stores
                .with(&profile, |store| store.space_id())
                .await?;
            if synced.is_some() {
                return Err(already_synced());
            }
            Ok(Some(profile))
        }
    }
}

/// « Arrêter de synchroniser ce profil avec <appareil> » : révocation dans l'espace.
pub(super) async fn stop_sharing<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    profile: P,
    device: DeviceKey,
) -> Result<()> {
    let now = shared.clock.timestamp();
    let changed = shared
        .stores
        .with(&profile, move |store| {
            // Ses collections partagées s'arrêtent avec le partage du profil
            store.drop_device(&device)?;
            store.revoke_device(&device, &now)
        })
        .await?;
    shared.offer_delivered(&device, &profile);
    shared.command(&device, Command::Announce);
    if changed {
        shared.request_push(profile, None);
    }
    Ok(())
}

/// « Retirer l'appareil » : révoqué dans la liste de confiance (définitif) et dans tous les
/// espaces dont il est membre ; sa session est fermée.
pub(super) async fn remove_device<P: ProfileKey>(
    shared: &Arc<Shared<P>>,
    device: DeviceKey,
) -> Result<()> {
    let now = shared.clock.timestamp();
    revoke_trust(shared, device, now.clone()).await?;
    let changed = shared
        .stores
        .run(move |profiles| {
            let mut changed = Vec::new();
            for profile in stores::enabled_profiles(profiles)? {
                in_store(profiles, &profile, |store| store.drop_device(&device))?;
                if in_store(profiles, &profile, |store| {
                    store.revoke_device(&device, &now)
                })? {
                    changed.push(profile);
                }
            }
            Ok(changed)
        })
        .await?;

    {
        let mut state = shared.state();
        state.outgoing_offers.remove(&device);
        state.incoming_offers.retain(|(key, _), _| *key != device);
        state.accepted_offers.retain(|(key, _), _| *key != device);
        if let Some(retry) = state.retries.remove(&device).and_then(|retry| retry.task) {
            retry.abort();
        }
    }
    shared.close_session(&device);
    for profile in changed {
        shared.request_push(profile, None);
    }
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
