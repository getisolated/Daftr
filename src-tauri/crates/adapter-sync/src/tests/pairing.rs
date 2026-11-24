//! Appairage : code correct ou non, appairages simultanés, renouvellement du code, expiration,
//! QR code, synchronisation complète aussitôt l'offre acceptée.

use std::time::Duration;

use app::ports::sync::{SyncEvent, SyncProfiles};
use domain::sync::{DeviceKey, DeviceKind, PairingCode};

use super::support::{
    contents, device, device_of_kind, eventually, link, share, space, write_note,
};
use crate::{Error, Fingerprint, MAX_FAILURES, MemoryNetwork, PairingState, PairingUri};

fn wrong(code: &PairingCode) -> PairingCode {
    let number: u32 = code.as_str().parse().unwrap();
    PairingCode::from_number((number + 1) % 1_000_000).unwrap()
}

/// Chaque appareil garde le type annoncé par l'autre dans `Hello`, dès l'appairage.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn linked_devices_remember_the_kind_of_each_other() {
    let network = MemoryNetwork::new();
    let a = device_of_kind(&network, "Bureau", DeviceKind::Desktop);
    let b = device_of_kind(&network, "Portable", DeviceKind::Laptop);
    a.create_space("travail", space(1));

    let ticket = a.engine.start_pairing().await.unwrap();
    b.engine.pair_with_code(ticket.code, None).await.unwrap();

    let kind_of = |device: &super::support::Device, other: DeviceKey| {
        device.trust.device(&other).map(|trusted| trusted.kind)
    };
    eventually("type de B connu de A", || {
        kind_of(&a, b.key()) == Some(DeviceKind::Laptop)
    })
    .await;
    eventually("type de A connu de B", || {
        kind_of(&b, a.key()) == Some(DeviceKind::Desktop)
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_right_code_links_both_devices_without_sharing_any_profile() {
    let network = MemoryNetwork::new();
    let (a, b) = (device(&network, "Bureau"), device(&network, "Portable"));
    let work = a.create_space("travail", space(1));
    write_note(&work, "compte rendu");

    let ticket = a.engine.start_pairing().await.unwrap();
    let linked = b
        .engine
        .pair_with_code(ticket.code.clone(), None)
        .await
        .unwrap();

    assert_eq!(linked.key, a.key());
    assert_eq!(linked.name, a.name);
    let b_key = b.key();
    let linked_to_b = |event: &SyncEvent<String>| matches!(event, SyncEvent::DeviceLinked { device, .. } if *device == b_key);
    eventually("liaison publiée", || a.has_event(linked_to_b)).await;
    assert!(a.trusts(&b) && b.trusts(&a));
    assert_eq!(*a.engine.pairing().borrow(), PairingState::Inactive);

    // La confiance seule ne partage rien : ni offre, ni profil créé chez B
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(b.profiles.profiles().unwrap().is_empty());
    assert!(!b.has_event(|event| matches!(event, SyncEvent::ProfileReceived { .. })));
    // Les deux appareils restent à l'écoute de l'autre
    assert!(a.engine.listening_port().is_some() && b.engine.listening_port().is_some());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sharing_a_profile_clones_it_on_the_other_device_without_asking() {
    let network = MemoryNetwork::new();
    let (a, b) = (device(&network, "Bureau"), device(&network, "Portable"));
    let work = a.create_space("travail", space(1));
    a.profiles.set_name("travail", "Travail");
    a.profiles
        .set_icon("travail", icon(b"\x89PNG\r\n\x1a\n-icone-"));
    write_note(&work, "compte rendu");
    write_note(&work, "liste 🥚 été");
    let ticket = a.engine.start_pairing().await.unwrap();
    b.engine.pair_with_code(ticket.code, None).await.unwrap();
    eventually("liés", || a.trusts(&b) && b.trusts(&a)).await;

    a.engine.offer("travail".to_owned(), b.key()).await.unwrap();

    let a_key = a.key();
    eventually("profil reçu", || {
        b.has_event(|event| {
            matches!(event, SyncEvent::ProfileReceived { name, device, .. }
                if name == "Travail" && *device == a_key)
        })
    })
    .await;
    eventually("synchronisation initiale", || {
        b.has_event(|event| matches!(event, SyncEvent::InitialSyncDone { .. }))
    })
    .await;
    let cloned = b.profiles.profile_for(&space(1)).unwrap().unwrap();
    assert_eq!(contents(&b.store(&cloned)), contents(&work));
    assert_eq!(b.profiles.name(&cloned).as_deref(), Some("Travail"));
    assert_eq!(
        b.profiles.icon(&cloned).map(|icon| icon.bytes),
        Some(b"\x89PNG\r\n\x1a\n-icone-".to_vec())
    );

    // Ensuite, le temps réel : une modification de B arrive chez A
    write_note(&b.store(&cloned), "écrit sur le portable");
    b.engine.local_change(&cloned);
    eventually("temps réel", || contents(&work).len() == 3).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_profile_without_a_name_arrives_with_the_default_name() {
    let network = MemoryNetwork::new();
    let (a, b) = (device(&network, "Bureau"), device(&network, "Portable"));
    a.create_space("default", space(1));
    link(&a, &b);
    a.engine.refresh().await.unwrap();
    b.engine.refresh().await.unwrap();

    a.engine.offer("default".to_owned(), b.key()).await.unwrap();

    eventually("profil reçu", || {
        b.has_event(|event| matches!(event, SyncEvent::ProfileReceived { .. }))
    })
    .await;
    let cloned = b.profiles.profile_for(&space(1)).unwrap().unwrap();
    // La doublure donne son nom par défaut à un profil sans nom
    assert_eq!(b.profiles.name(&cloned).as_deref(), Some("Default profile"));
    assert!(b.profiles.icon(&cloned).is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_space_already_present_is_not_cloned_again() {
    let network = MemoryNetwork::new();
    let (a, b) = (device(&network, "Bureau"), device(&network, "Portable"));
    let work = a.create_space("travail", space(1));
    write_note(&work, "une note");
    share(&a, "travail", &b, "existant");
    link(&a, &b);
    a.engine.refresh().await.unwrap();
    b.engine.refresh().await.unwrap();

    a.engine.offer("travail".to_owned(), b.key()).await.unwrap();

    eventually("synchronisé dans le profil existant", || {
        contents(&b.store("existant")) == ["une note"]
    })
    .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(b.profiles.profiles().unwrap(), ["existant"]);
    assert!(!b.has_event(|event| matches!(event, SyncEvent::ProfileReceived { .. })));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unsharing_a_profile_stops_the_exchange_and_sharing_again_resumes_it() {
    let network = MemoryNetwork::new();
    let (a, b) = (device(&network, "Bureau"), device(&network, "Portable"));
    let work = a.create_space("travail", space(1));
    write_note(&work, "avant");
    link(&a, &b);
    a.engine.refresh().await.unwrap();
    b.engine.refresh().await.unwrap();
    a.engine.offer("travail".to_owned(), b.key()).await.unwrap();
    eventually("profil cloné", || {
        b.profiles.profile_for(&space(1)).unwrap().is_some()
    })
    .await;
    let cloned = b.profiles.profile_for(&space(1)).unwrap().unwrap();
    eventually("contenu reçu", || contents(&b.store(&cloned)) == ["avant"]).await;

    a.engine
        .stop_sharing("travail".to_owned(), b.key())
        .await
        .unwrap();
    write_note(&work, "pendant");
    a.engine.local_change(&"travail".to_owned());
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(contents(&b.store(&cloned)), ["avant"]);

    a.engine.offer("travail".to_owned(), b.key()).await.unwrap();
    eventually("échange repris dans le même profil", || {
        contents(&b.store(&cloned)) == ["avant", "pendant"]
    })
    .await;
    assert_eq!(b.profiles.profiles().unwrap(), [cloned]);
}

fn icon(bytes: &[u8]) -> app::ports::sync::ProfileIcon {
    app::ports::sync::ProfileIcon {
        format: domain::profile::IconFormat::Png,
        bytes: bytes.to_vec(),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_wrong_code_is_refused_on_both_sides() {
    let network = MemoryNetwork::new();
    let (a, b) = (device(&network, "Bureau"), device(&network, "Portable"));
    a.create_space("travail", space(1));
    let ticket = a.engine.start_pairing().await.unwrap();

    let result = b
        .engine
        .pair_with_code(wrong(&ticket.code), Some(a.addr()))
        .await;

    assert!(matches!(result, Err(Error::WrongCode)), "{result:?}");
    assert!(!a.trusts(&b) && !b.trusts(&a));
    assert!(a.engine.connected_devices().is_empty());
    assert!(matches!(
        &*a.engine.pairing().borrow(),
        PairingState::Active(_)
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn simultaneous_pairings_with_different_codes_reach_the_right_devices() {
    let network = MemoryNetwork::new();
    let (a1, a2) = (device(&network, "Bureau 1"), device(&network, "Bureau 2"));
    let (b1, b2) = (
        device(&network, "Portable 1"),
        device(&network, "Portable 2"),
    );
    a1.create_space("p", space(1));
    a2.create_space("p", space(2));
    let ticket1 = a1.engine.start_pairing().await.unwrap();
    let ticket2 = a2.engine.start_pairing().await.unwrap();

    let (linked1, linked2) = tokio::join!(
        b1.engine.pair_with_code(ticket1.code, None),
        b2.engine.pair_with_code(ticket2.code, None)
    );

    assert_eq!(linked1.unwrap().key, a1.key());
    assert_eq!(linked2.unwrap().key, a2.key());
    eventually("liaisons", || a1.trusts(&b1) && a2.trusts(&b2)).await;
    assert!(!a1.trusts(&b2) && !a2.trusts(&b1));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ten_failures_renew_the_code() {
    let network = MemoryNetwork::new();
    let (a, b) = (device(&network, "Bureau"), device(&network, "Portable"));
    a.create_space("p", space(1));
    let first = a.engine.start_pairing().await.unwrap();
    let pairing = a.engine.pairing();

    for _ in 0..MAX_FAILURES {
        let result = b
            .engine
            .pair_with_code(wrong(&first.code), Some(a.addr()))
            .await;
        assert!(matches!(result, Err(Error::WrongCode)), "{result:?}");
    }

    let PairingState::Active(renewed) = pairing.borrow().clone() else {
        panic!("le mode appairage devrait rester ouvert");
    };
    assert_ne!(renewed.code, first.code);
    assert_ne!(renewed.qr_svg, first.qr_svg);
    let old = b.engine.pair_with_code(first.code, Some(a.addr())).await;
    assert!(matches!(old, Err(Error::WrongCode)), "{old:?}");
    let linked = b.engine.pair_with_code(renewed.code, Some(a.addr())).await;
    assert_eq!(linked.unwrap().key, a.key());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_expired_mode_is_no_longer_announced_and_refuses_pairing() {
    let network = MemoryNetwork::new();
    let (a, b) = (device(&network, "Bureau"), device(&network, "Portable"));
    a.create_space("p", space(1));
    let ticket = a.engine.start_pairing().await.unwrap();
    let announced = a.discovery.announcement().unwrap();
    assert_eq!(announced.pairing.as_ref(), Some(&a.name));

    a.clock.advance(Duration::from_secs(301));
    let result = b.engine.pair_with_code(ticket.code, Some(a.addr())).await;

    assert!(matches!(result, Err(Error::NotPairing)), "{result:?}");
    assert_eq!(*a.engine.pairing().borrow(), PairingState::Inactive);
    eventually("annonce sans appairage", || {
        a.discovery
            .announcement()
            .is_some_and(|announcement| announcement.pairing.is_none())
    })
    .await;
    assert!(!a.trusts(&b));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_qr_code_with_another_fingerprint_is_refused() {
    let network = MemoryNetwork::new();
    let (a, b) = (device(&network, "Bureau"), device(&network, "Portable"));
    a.create_space("p", space(1));
    let ticket = a.engine.start_pairing().await.unwrap();

    let forged = PairingUri {
        fingerprint: Fingerprint::of(&DeviceKey::from_bytes([9; 32])),
        ..ticket.uri.clone()
    };
    let result = b.engine.pair_with_qr(&forged.to_string()).await;
    assert!(
        matches!(result, Err(Error::FingerprintMismatch)),
        "{result:?}"
    );
    assert!(!a.trusts(&b));

    let linked = b
        .engine
        .pair_with_qr(&ticket.uri.to_string())
        .await
        .unwrap();
    assert_eq!(linked.key, a.key());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn pairing_without_any_device_in_pairing_mode_finds_nothing() {
    let network = MemoryNetwork::new();
    let (a, b) = (device(&network, "Bureau"), device(&network, "Portable"));
    a.create_space("p", space(1));
    a.engine.refresh().await.unwrap();

    let code = PairingCode::parse("123456").unwrap();
    let result = b.engine.pair_with_code(code, None).await;

    assert!(matches!(result, Err(Error::NoPairingDevice)), "{result:?}");
    assert!(
        !b.discovery.is_browsing(),
        "la recherche temporaire doit s'arrêter"
    );
}
