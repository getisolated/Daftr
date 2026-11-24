//! Entrées « à configurer » dans la session : collections proposées par l'appareil distant,
//! gardées hors des bases de profil jusqu'à leur activation.

use std::collections::HashSet;

use app::ports::collection_sync::{CollectionInbox, PendingCollection};
use app::ports::sync::{CollectionOffer, SyncEvent};
use domain::sync::{DeviceKey, SpaceId};
use domain::uuid::Uuid;

use crate::error::{Error, Result};
use crate::session::Session;
use crate::stores::ProfileKey;
use crate::wire::{MAX_OFFERED, Message, OfferedCollection};

impl<P: ProfileKey> Session<P> {
    /// Dit à l'appareil distant quelles collections reçues de lui attendent encore ici, une
    /// fois par session : il répond `ShareStop` pour celles qu'il ne propose plus (arrêtées
    /// pendant que cet appareil était absent).
    pub(in crate::session) async fn send_pending_list(&mut self) -> Result<()> {
        if std::mem::replace(&mut self.pending_sent, true) {
            return Ok(());
        }
        let remote = self.remote;
        let pending = self
            .shared
            .with_inbox(move |inbox| {
                Ok(inbox
                    .pending()?
                    .into_iter()
                    .filter(|entry| entry.device == remote && !entry.ignored)
                    .map(|entry| *entry.offer.share.as_bytes())
                    .collect::<Vec<_>>())
            })
            .await?;
        if pending.is_empty() {
            return Ok(());
        }
        self.send(&Message::CollectionPending { shares: pending })
            .await
    }

    /// Collections proposées par l'appareil distant : gardées comme entrées « à configurer »
    /// (rien n'est écrit sur le disque, aucune question n'interrompt l'utilisateur). Une
    /// collection déjà activée (réponse perdue) l'est d'office, une proposition ignée est
    /// refusée à nouveau.
    pub(in crate::session) async fn on_collection_offer(
        &mut self,
        entries: Vec<OfferedCollection>,
    ) -> Result<()> {
        if entries.len() > MAX_OFFERED {
            return Err(Error::Malformed);
        }
        let Some(name) = self.remote_name.clone() else {
            return Err(Error::Unexpected);
        };
        let known = self.shares_with_remote().await?;
        let mut fresh = Vec::new();

        for entry in entries {
            let share = Uuid::from_bytes(entry.share);
            let accepted = known
                .iter()
                .any(|(_, s)| s.sync_id == share && s.is_active_with(&self.remote));
            if accepted {
                // Réponse perdue ou ordre perdu avec une session remplacée : on reprend le partage
                self.reply_collection(entry.share, true).await?;
                self.sync_collections().await?;
                continue;
            }
            let offer = offer_of(&entry, share);
            let device = self.remote;
            let stored = offer.clone();
            let outcome = self
                .shared
                .with_inbox(move |inbox| store_offer(inbox, device, stored))
                .await?;
            match outcome {
                Stored::New => fresh.push(offer),
                Stored::Known => {}
                Stored::Declined => self.reply_collection(entry.share, false).await?,
            }
        }
        if !fresh.is_empty() {
            let device = self.remote;
            self.shared.publish(SyncEvent::CollectionOfferReceived {
                device,
                name,
                offers: fresh,
            });
        }
        Ok(())
    }

    /// Les collections que l'appareil distant a reçues de nous sans les activer : celles que
    /// nous ne partageons plus avec lui reçoivent `ShareStop`.
    pub(in crate::session) async fn on_collection_pending(
        &mut self,
        shares: Vec<[u8; 16]>,
    ) -> Result<()> {
        if shares.len() > MAX_OFFERED {
            return Err(Error::Malformed);
        }
        let known: HashSet<Uuid> = self
            .shares_with_remote()
            .await?
            .into_iter()
            .map(|(_, share)| share.sync_id)
            .collect();
        for share in shares {
            if !known.contains(&Uuid::from_bytes(share)) {
                self.send(&Message::ShareStop { share }).await?;
            }
        }
        Ok(())
    }
}

/// L'entrée « à configurer » d'une collection reçue : champs assainis, nom de repli.
fn offer_of(entry: &OfferedCollection, share: Uuid) -> CollectionOffer {
    let name = entry.clean_name();
    CollectionOffer {
        share,
        source_profile_id: entry.clean_profile_id(),
        source_profile_name: entry.clean_profile_name(),
        source_space: entry.space.map(SpaceId::from_bytes),
        name: if name.is_empty() {
            "Collection".to_owned()
        } else {
            name
        },
        files: entry.files,
        bytes: entry.bytes,
    }
}

/// Ce que l'entrée reçue est devenue dans la liste des collections en attente.
enum Stored {
    /// Nouvelle entrée.
    New,
    /// Déjà dans la liste (mise à jour : nombre de fichiers, nom du profil source).
    Known,
    /// Ignorée par l'utilisateur : retirée, le refus repart.
    Declined,
}

fn store_offer(
    inbox: &dyn CollectionInbox,
    device: DeviceKey,
    offer: CollectionOffer,
) -> app::AppResult<Stored> {
    let existing = inbox
        .pending()?
        .into_iter()
        .find(|entry| entry.device == device && entry.offer.share == offer.share);
    match existing {
        Some(entry) if entry.ignored => {
            inbox.remove(&device, &offer.share)?;
            Ok(Stored::Declined)
        }
        Some(_) => {
            inbox.put(&PendingCollection {
                device,
                offer,
                ignored: false,
            })?;
            Ok(Stored::Known)
        }
        None => {
            inbox.put(&PendingCollection {
                device,
                offer,
                ignored: false,
            })?;
            Ok(Stored::New)
        }
    }
}
