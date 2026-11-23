//! Sécurité : adresses non locales, clés inconnues ou révoquées, espaces jamais révélés,
//! messages trop gros, pause puis reprise.

use std::net::SocketAddr;

use app::ports::sync::{SyncStatus, SyncStore};
use app::ports::trust::{TrustOrigin, TrustStore, TrustedDevice};
use domain::sync::{DeviceKind, DeviceName, PairingCode, SpaceId};
use tokio::net::TcpStream;

use super::support::{
    Device, NOW, contents, device, device_with, eventually, link, share, space, write_note,
};
use crate::framing::MAX_MESSAGE_BYTES;
use crate::noise::test_support::send_forged_length;
use crate::noise::{self, Channel, Role};
use crate::wire::{Message, SessionKind};
use crate::{AddrPolicy, DeviceIdentity, Error, MemoryNetwork};

/// Un client brut qui ouvre une session `Sync` avec l'identité `identity`.
async fn open_sync(identity: &DeviceIdentity, addr: SocketAddr) -> Channel<TcpStream> {
    let stream = TcpStream::connect(addr).await.unwrap();
    let mut channel = noise::handshake(stream, identity, Role::Initiator)
        .await
        .unwrap();
    channel
        .send(&Message::Open(SessionKind::Sync))
        .await
        .unwrap();
    channel
}

/// Fait confiance à une identité brute, membre de l'espace du profil `profile`.
fn admit(device: &Device, identity: &DeviceIdentity, profile: Option<&str>) {
    let name = DeviceName::parse("Client").unwrap();
    device
        .trust
        .put_device(&TrustedDevice {
            key: identity.key(),
            name: name.clone(),
            kind: DeviceKind::Unknown,
            origin: TrustOrigin::Paired,
            added_at: NOW.to_owned(),
            revoked_at: None,
        })
        .unwrap();
    if let Some(profile) = profile {
        let store = device.store(profile);
        store.add_device(&identity.key(), &name, NOW).unwrap();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_non_local_source_is_closed_before_the_handshake() {
    let network = MemoryNetwork::new();
    // Politique de l'application : loopback refusé comme toute adresse hors réseau local
    let a = device_with(&network, "A", AddrPolicy::Lan);
    a.create_space("p", space(1));
    a.engine.refresh().await.unwrap();
    let client = DeviceIdentity::generate().unwrap();

    let stream = TcpStream::connect(a.addr()).await.unwrap();
    let handshake = noise::handshake(stream, &client, Role::Initiator).await;

    assert!(
        handshake.is_err(),
        "le handshake n'aurait pas dû avoir lieu"
    );
    assert!(a.engine.connected_devices().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_non_local_address_is_never_dialed() {
    let network = MemoryNetwork::new();
    let b = device(&network, "B");
    let public: SocketAddr = "8.8.8.8:47820".parse().unwrap();
    let code = PairingCode::parse("123456").unwrap();

    let result = b.engine.pair_with_code(code, Some(public)).await;
    assert!(
        matches!(result, Err(Error::NotLocal(ip)) if ip == public.ip()),
        "{result:?}"
    );
    let qr = format!("daftr-pair:1:{public}:{}:123456", b.identity.fingerprint());
    let result = b.engine.pair_with_qr(&qr).await;
    assert!(matches!(result, Err(Error::NotLocal(_))), "{result:?}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unknown_and_revoked_keys_are_refused() {
    let network = MemoryNetwork::new();
    let (a, b) = (device(&network, "A"), device(&network, "B"));
    a.create_space("p", space(1));
    share(&a, "p", &b, "p");
    link(&a, &b);
    a.engine.refresh().await.unwrap();

    let stranger = DeviceIdentity::generate().unwrap();
    let mut channel = open_sync(&stranger, a.addr()).await;
    assert!(channel.recv().await.is_err(), "clé inconnue acceptée");

    b.engine.refresh().await.unwrap();
    eventually("A et B connectés", || {
        a.engine.connected_devices() == [b.key()]
    })
    .await;
    a.engine.remove_device(b.key()).await.unwrap();

    eventually("B déconnecté", || b.engine.connected_devices().is_empty()).await;
    assert!(!a.trusts(&b));
    let revoked = a.store("p").devices().unwrap();
    assert!(
        revoked
            .iter()
            .any(|d| d.key == b.key() && d.revoked_at.is_some())
    );
    let mut channel = open_sync(&b.identity, a.addr()).await;
    assert!(channel.recv().await.is_err(), "clé révoquée acceptée");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_unshared_space_is_never_revealed_in_hello() {
    let network = MemoryNetwork::new();
    let a = device(&network, "A");
    a.create_space("shared", space(1));
    a.create_space("private", space(2));
    a.create_space("disabled", space(3));
    let client = DeviceIdentity::generate().unwrap();
    admit(&a, &client, Some("shared"));
    admit(&a, &client, Some("disabled"));
    a.store("disabled").set_enabled(false);
    a.engine.refresh().await.unwrap();

    let mut channel = open_sync(&client, a.addr()).await;
    let Message::Hello(hello) = channel.recv().await.unwrap() else {
        panic!("Hello attendu");
    };

    let spaces: Vec<SpaceId> = hello.spaces.into_iter().map(SpaceId::from_bytes).collect();
    assert_eq!(spaces, [space(1)]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_oversized_message_closes_the_session() {
    let network = MemoryNetwork::new();
    let a = device(&network, "A");
    a.create_space("p", space(1));
    let client = DeviceIdentity::generate().unwrap();
    admit(&a, &client, Some("p"));
    a.engine.refresh().await.unwrap();

    let channel = open_sync(&client, a.addr()).await;
    let (mut reader, mut writer) = channel.split();
    assert!(matches!(reader.recv().await, Ok(Message::Hello(_))));
    eventually("session ouverte", || {
        a.engine.connected_devices() == [client.key()]
    })
    .await;

    let length = u32::try_from(MAX_MESSAGE_BYTES + 1).unwrap();
    send_forged_length(&mut writer, length).await;

    eventually("session fermée", || {
        a.engine.connected_devices().is_empty()
    })
    .await;
    assert!(reader.recv().await.is_err());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pause_stops_listening_and_announcing_then_resume_catches_up() {
    let network = MemoryNetwork::new();
    let (a, b) = (device(&network, "A"), device(&network, "B"));
    let mine = a.create_space("p", space(1));
    share(&a, "p", &b, "p");
    link(&a, &b);
    a.engine.refresh().await.unwrap();
    b.engine.refresh().await.unwrap();
    eventually("connectés", || a.engine.connected_devices() == [b.key()]).await;
    let port = a.addr();

    a.engine.set_paused(true).await.unwrap();

    assert_eq!(a.engine.listening_port(), None);
    assert_eq!(a.discovery.announcement(), None);
    assert!(!a.discovery.is_browsing());
    assert_eq!(a.engine.status(), SyncStatus::Paused);
    assert!(
        TcpStream::connect(port).await.is_err(),
        "l'écoute devrait être fermée"
    );
    eventually("B déconnecté", || b.engine.connected_devices().is_empty()).await;

    write_note(&mine, "pendant la pause (A)");
    write_note(&b.store("p"), "pendant la pause (B)");
    a.engine.local_change(&"p".to_owned());
    a.engine.set_paused(false).await.unwrap();

    let expected = ["pendant la pause (A)", "pendant la pause (B)"];
    eventually("rattrapage à la reprise", || {
        contents(&mine) == expected && contents(&b.store("p")) == expected
    })
    .await;
}
