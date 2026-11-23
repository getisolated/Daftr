//! Profils et sessions : espaces communs seulement, une connexion par paire, profil désactivé,
//! propagation transitive, offre à un appareil absent, rien qui tourne sans profil.

use app::ports::sync::{SyncEvent, SyncProfiles, SyncStatus};

use super::support::{contents, device, eventually, link, share, space, write_note};
use crate::{MemoryNetwork, OfferChoice};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn devices_exchange_only_their_common_spaces_on_one_connection() {
    let network = MemoryNetwork::new();
    let (a, b, c) = (
        device(&network, "A"),
        device(&network, "B"),
        device(&network, "C"),
    );
    // A (P1, P2), B (Q1), C (P1, P2, Q1) ; A et B ne se connaissent pas
    let a1 = a.create_space("p1", space(1));
    let a2 = a.create_space("p2", space(2));
    let b1 = b.create_space("q1", space(3));
    share(&a, "p1", &c, "p1");
    share(&a, "p2", &c, "p2");
    share(&b, "q1", &c, "q1");
    link(&a, &c);
    link(&b, &c);
    write_note(&a1, "a-p1");
    write_note(&a2, "a-p2");
    write_note(&b1, "b-q1");
    write_note(&c.store("p1"), "c-p1");
    write_note(&c.store("q1"), "c-q1");

    for device in [&a, &b, &c] {
        device.engine.refresh().await.unwrap();
    }

    eventually("P1 et P2 entre A et C", || {
        contents(&a1) == ["a-p1", "c-p1"]
            && contents(&c.store("p1")) == ["a-p1", "c-p1"]
            && contents(&c.store("p2")) == ["a-p2"]
    })
    .await;
    eventually("Q1 entre B et C", || {
        contents(&b1) == ["b-q1", "c-q1"] && contents(&c.store("q1")) == ["b-q1", "c-q1"]
    })
    .await;
    assert_eq!(a.engine.connected_devices(), [c.key()]);
    assert_eq!(b.engine.connected_devices(), [c.key()]);
    let mut connected = c.engine.connected_devices();
    connected.sort();
    let mut expected = vec![a.key(), b.key()];
    expected.sort();
    assert_eq!(connected, expected);
    assert_eq!(contents(&a2), ["a-p2"]);
    eventually("à jour avec C", || {
        a.engine.status() == SyncStatus::UpToDate { devices: 1 }
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn simultaneous_dials_keep_a_single_connection() {
    let network = MemoryNetwork::new();
    let (a, b) = (device(&network, "A"), device(&network, "B"));
    let shared = a.create_space("p", space(1));
    share(&a, "p", &b, "p");
    link(&a, &b);
    a.engine.refresh().await.unwrap();
    b.engine.refresh().await.unwrap();

    let (first, second) = tokio::join!(a.engine.sync_now(), b.engine.sync_now());
    first.unwrap();
    second.unwrap();
    write_note(&shared, "après");
    a.engine.local_change(&"p".to_owned());

    eventually("modification reçue", || {
        contents(&b.store("p")) == ["après"]
    })
    .await;
    assert_eq!(a.engine.connected_devices(), [b.key()]);
    assert_eq!(b.engine.connected_devices(), [a.key()]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_disabled_profile_is_ignored() {
    let network = MemoryNetwork::new();
    let (a, c) = (device(&network, "A"), device(&network, "C"));
    let a1 = a.create_space("p1", space(1));
    let a2 = a.create_space("p2", space(2));
    share(&a, "p1", &c, "p1");
    share(&a, "p2", &c, "p2");
    link(&a, &c);
    c.store("p2").set_enabled(false);
    write_note(&a2, "jamais chez C");
    write_note(&a1, "chez C");

    a.engine.refresh().await.unwrap();
    c.engine.refresh().await.unwrap();

    eventually("P1 synchronisé", || contents(&c.store("p1")) == ["chez C"]).await;
    // Un second échange sur P1 : P2, annoncé dans le même Hello, serait passé avant
    write_note(&a1, "encore");
    a.engine.local_change(&"p1".to_owned());
    a.engine.local_change(&"p2".to_owned());
    eventually("second échange", || contents(&c.store("p1")).len() == 2).await;
    assert!(contents(&c.store("p2")).is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn changes_propagate_from_a_to_c_through_b() {
    let network = MemoryNetwork::new();
    let (a, b, c) = (
        device(&network, "A"),
        device(&network, "B"),
        device(&network, "C"),
    );
    let origin = a.create_space("p", space(1));
    share(&a, "p", &b, "p");
    share(&b, "p", &c, "p");
    // A et C ne sont pas appairés
    link(&a, &b);
    link(&b, &c);
    write_note(&origin, "écrit sur A");

    a.engine.refresh().await.unwrap();
    b.engine.refresh().await.unwrap();
    eventually("A vers B", || contents(&b.store("p")) == ["écrit sur A"]).await;
    a.engine.shutdown().await;

    c.engine.refresh().await.unwrap();
    eventually("B vers C", || contents(&c.store("p")) == ["écrit sur A"]).await;
    assert!(!c.engine.connected_devices().contains(&a.key()));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_offer_to_an_absent_device_is_sent_when_it_appears() {
    let network = MemoryNetwork::new();
    let (a, b) = (device(&network, "A"), device(&network, "B"));
    let offered = a.create_space("p", space(1));
    write_note(&offered, "proposée");
    b.create_space("perso", space(2));
    link(&a, &b);
    b.engine.set_paused(true).await.unwrap();
    a.engine.refresh().await.unwrap();

    a.engine.offer("p".to_owned(), b.key()).await.unwrap();
    assert!(a.engine.connected_devices().is_empty());

    b.engine.set_paused(false).await.unwrap();
    eventually("offre reçue à l'apparition", || {
        b.has_event(
            |event| matches!(event, SyncEvent::OfferReceived { device, .. } if *device == a.key()),
        )
    })
    .await;
    b.engine
        .answer_offer(a.key(), space(1), OfferChoice::NewProfile)
        .await
        .unwrap();

    eventually("synchronisation après acceptation", || {
        b.has_event(|event| matches!(event, SyncEvent::InitialSyncDone { .. }))
    })
    .await;
    let joined = b.profiles.profile_for(&space(1)).unwrap().unwrap();
    assert_eq!(contents(&b.store(&joined)), ["proposée"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_new_port_reopens_the_listener_and_keeps_syncing() {
    let network = MemoryNetwork::new();
    let (a, b) = (device(&network, "A"), device(&network, "B"));
    let shared = a.create_space("p", space(1));
    share(&a, "p", &b, "p");
    link(&a, &b);
    a.engine.refresh().await.unwrap();
    b.engine.refresh().await.unwrap();
    let free = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = free.local_addr().unwrap().port();
    drop(free);

    a.engine.set_port(port).await.unwrap();

    assert_eq!(a.engine.listening_port(), Some(port));
    assert_eq!(a.discovery.announcement().unwrap().port, port);
    b.engine.sync_now().await.unwrap();
    write_note(&shared, "après le changement de port");
    a.engine.local_change(&"p".to_owned());
    eventually("modification reçue", || {
        contents(&b.store("p")) == ["après le changement de port"]
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn nothing_runs_without_a_synchronized_profile() {
    let network = MemoryNetwork::new();
    let a = device(&network, "A");

    a.engine.refresh().await.unwrap();

    assert_eq!(a.engine.listening_port(), None);
    assert_eq!(a.discovery.announcement(), None);
    assert!(!a.discovery.is_browsing());
    assert_eq!(a.engine.status(), SyncStatus::Disabled);

    a.create_space("p", space(1));
    a.engine.refresh().await.unwrap();
    assert!(a.engine.listening_port().is_some());
    assert!(a.discovery.announcement().is_some());
    assert_eq!(a.engine.status(), SyncStatus::NoDevice);
}
