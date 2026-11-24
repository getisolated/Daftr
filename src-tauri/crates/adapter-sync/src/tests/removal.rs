//! Retrait d'un appareil : l'appareil retiré l'apprend (`Removed`) et coupe le lien de son
//! côté, sans rien supprimer de ses notes ni de ses fichiers.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use app::ports::collection_sync::{CollectionInbox, CollectionSyncStore};
use app::ports::sync::{SyncEvent, SyncStore};
use app::ports::trust::{TrustOrigin, TrustStore, TrustedDevice};
use domain::sync::{DeviceKind, DeviceName};
use tokio::net::TcpStream;

use super::support::{Device, NOW, contents, device, eventually, link, share, space, write_note};
use crate::noise::{self, Channel, Role};
use crate::wire::{FEATURE_REMOVED, Hello, Message, PROTOCOL_VERSION, SessionKind};
use crate::{DeviceIdentity, MemoryNetwork};

fn removed_events(device: &Device) -> usize {
    device
        .events()
        .iter()
        .filter(|event| matches!(event, SyncEvent::DeviceRemoved { .. }))
        .count()
}

/// A et B liés, un profil partagé avec une note de chaque côté, les deux moteurs démarrés.
async fn connected_pair() -> (Device, Device, Arc<MemoryNetwork>) {
    let network = MemoryNetwork::new();
    let (a, b) = (device(&network, "A"), device(&network, "B"));
    let store = a.create_space("p", space(1));
    share(&a, "p", &b, "p");
    link(&a, &b);
    write_note(&store, "note de A");
    write_note(&b.store("p"), "note de B");
    a.engine.refresh().await.unwrap();
    b.engine.refresh().await.unwrap();
    eventually("notes échangées", || {
        contents(&a.store("p")) == ["note de A", "note de B"]
            && contents(&b.store("p")) == ["note de A", "note de B"]
    })
    .await;
    (a, b, network)
}

/// Un client brut qui ouvre une session `Sync` avec `identity`.
async fn open_sync(identity: &DeviceIdentity, device: &Device) -> Channel<TcpStream> {
    let stream = TcpStream::connect(device.addr()).await.unwrap();
    let mut channel = noise::handshake(stream, identity, Role::Initiator)
        .await
        .unwrap();
    channel
        .send(&Message::Open(SessionKind::Sync))
        .await
        .unwrap();
    channel
}

/// Lit jusqu'à la fermeture et dit si `Removed` est passé.
async fn drain_told(channel: &mut Channel<TcpStream>) -> bool {
    let mut told = false;
    while let Ok(message) = channel.recv().await {
        told |= message == Message::Removed;
    }
    told
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_removed_device_connected_at_the_time_unlinks_itself() {
    let (a, b, _network) = connected_pair().await;
    // Une collection partagée dans chaque sens : tout doit disparaître chez A
    let root = PathBuf::from("a-docs");
    a.store("p")
        .list_collection(&root.to_string_lossy(), "Docs");
    a.disk.create_directory(&root);
    let offered = a
        .engine
        .share_collection(
            "p".to_owned(),
            root.to_string_lossy().into_owned(),
            vec![b.key()],
        )
        .await
        .unwrap()
        .sync_id;
    let other = PathBuf::from("b-docs");
    b.store("p")
        .list_collection(&other.to_string_lossy(), "Notes B");
    b.disk.create_directory(&other);
    b.engine
        .share_collection(
            "p".to_owned(),
            other.to_string_lossy().into_owned(),
            vec![a.key()],
        )
        .await
        .unwrap();
    eventually("entrée à configurer chez A", || {
        !a.inbox.pending().unwrap().is_empty()
    })
    .await;
    assert!(a.store("p").share(&offered).unwrap().is_some());

    b.engine.remove_device(a.key()).await.unwrap();

    eventually("A retiré chez lui aussi", || !a.trusts(&b)).await;
    eventually("événement chez A", || removed_events(&a) == 1).await;
    assert!(a.has_event(|event| matches!(event, SyncEvent::DeviceRemoved { device, name }
        if *device == b.key() && name.as_str() == "B")));
    let devices = a.store("p").devices().unwrap();
    assert!(
        devices
            .iter()
            .any(|device| device.key == b.key() && device.revoked_at.is_some())
    );
    assert!(a.store("p").share(&offered).unwrap().is_none());
    assert!(a.inbox.pending().unwrap().is_empty());
    eventually("sessions fermées", || {
        a.engine.connected_devices().is_empty() && b.engine.connected_devices().is_empty()
    })
    .await;
    // Rien n'est supprimé chez A
    assert_eq!(contents(&a.store("p")), ["note de A", "note de B"]);
    assert!(a.has_event(|event| matches!(event, SyncEvent::CollectionPendingChanged)));
    assert!(a.has_event(|event| matches!(event, SyncEvent::CollectionChanged { .. })));
    // B, qui a retiré A, n'est pas « retiré » par A : aucun rebond
    assert_eq!(removed_events(&b), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_device_offline_at_the_time_learns_it_when_it_reconnects() {
    let network = MemoryNetwork::new();
    let (a, b) = (device(&network, "A"), device(&network, "B"));
    let store = a.create_space("p", space(1));
    share(&a, "p", &b, "p");
    link(&a, &b);
    write_note(&store, "note de A");
    // A n'est pas démarré : B le retire sans session
    b.engine.refresh().await.unwrap();
    b.engine.remove_device(a.key()).await.unwrap();
    assert!(a.trusts(&b));

    a.engine.refresh().await.unwrap();
    a.engine.sync_now().await.unwrap();

    eventually("A apprend son retrait", || !a.trusts(&b)).await;
    eventually("événement chez A", || removed_events(&a) == 1).await;
    assert_eq!(contents(&a.store("p")), ["note de A"]);
    assert_eq!(removed_events(&b), 0);
    // A ne retente plus B
    a.engine.sync_now().await.unwrap();
    assert!(a.engine.connected_devices().is_empty());
    assert_eq!(removed_events(&a), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn only_a_known_revoked_key_is_told_never_a_stranger() {
    let (a, b, _network) = connected_pair().await;
    let stranger = DeviceIdentity::generate().unwrap();
    let mut channel = open_sync(&stranger, &b).await;
    assert!(!drain_told(&mut channel).await, "clé inconnue informée");

    // Clé retirée, mais qui n'a pas encore reçu l'information (session coupée à la main)
    let ghost = DeviceIdentity::generate().unwrap();
    b.trust
        .put_device(&TrustedDevice {
            key: ghost.key(),
            name: DeviceName::parse("Fantôme").unwrap(),
            kind: DeviceKind::Unknown,
            origin: TrustOrigin::Paired,
            added_at: NOW.to_owned(),
            revoked_at: Some(NOW.to_owned()),
        })
        .unwrap();
    let mut channel = open_sync(&ghost, &b).await;
    assert!(drain_told(&mut channel).await, "clé révoquée non informée");
    assert!(b.trusts(&a));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_removal_never_bounces_back() {
    let (a, b, _network) = connected_pair().await;
    b.engine.remove_device(a.key()).await.unwrap();
    eventually("A délié", || removed_events(&a) == 1).await;

    // A recompose B : B répond `Removed`, A ne le prend plus en compte (plus de confiance)
    let mut channel = open_sync(&a.identity, &b).await;
    assert_eq!(channel.recv().await.unwrap(), Message::Removed);
    drop(channel);
    a.engine.sync_now().await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;

    assert_eq!(removed_events(&a), 1);
    assert_eq!(removed_events(&b), 0);
    assert!(a.engine.connected_devices().is_empty());
    assert!(b.engine.connected_devices().is_empty());
    assert_eq!(contents(&a.store("p")), ["note de A", "note de B"]);
    assert_eq!(contents(&b.store("p")), ["note de A", "note de B"]);
}

/// Un appareil brut, membre de l'espace de B, qui annonce `features` ; rend le canal une fois
/// la session de B établie (B a lu son `Hello` et commence le rattrapage).
async fn raw_peer(b: &Device, features: u8) -> (DeviceIdentity, Channel<TcpStream>) {
    let identity = DeviceIdentity::generate().unwrap();
    let name = DeviceName::parse("Ancien").unwrap();
    b.trust
        .put_device(&TrustedDevice {
            key: identity.key(),
            name: name.clone(),
            kind: DeviceKind::Unknown,
            origin: TrustOrigin::Paired,
            added_at: NOW.to_owned(),
            revoked_at: None,
        })
        .unwrap();
    let store = b.store("p");
    store.add_device(&identity.key(), &name, NOW).unwrap();
    let space = store.space_id().unwrap().unwrap();
    b.engine.refresh().await.unwrap();

    let mut channel = open_sync(&identity, b).await;
    assert!(matches!(channel.recv().await.unwrap(), Message::Hello(_)));
    channel
        .send(&Message::Hello(Hello {
            protocol: PROTOCOL_VERSION,
            name: "Ancien".to_owned(),
            spaces: vec![*space.as_bytes()],
            kind: DeviceKind::Unknown,
            features,
        }))
        .await
        .unwrap();
    loop {
        if matches!(channel.recv().await.unwrap(), Message::Changed { .. }) {
            return (identity, channel);
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn removed_is_sent_only_to_a_peer_that_announces_it() {
    let network = MemoryNetwork::new();
    let b = device(&network, "B");
    b.create_space("p", space(1));

    // Version antérieure : pas de bit, rien n'est envoyé, la session se ferme
    let (old, mut channel) = raw_peer(&b, 0).await;
    b.engine.remove_device(old.key()).await.unwrap();
    assert!(!drain_told(&mut channel).await, "message envoyé sans le bit");

    // Version qui l'annonce : elle le reçoit
    let (recent, mut channel) = raw_peer(&b, FEATURE_REMOVED).await;
    b.engine.remove_device(recent.key()).await.unwrap();
    assert!(drain_told(&mut channel).await, "Removed non reçu");
}
