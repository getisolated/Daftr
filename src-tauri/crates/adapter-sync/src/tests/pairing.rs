//! Appairage : code correct ou non, appairages simultanés, renouvellement du code, expiration,
//! QR code, synchronisation complète aussitôt l'offre acceptée.

use std::time::Duration;

use app::ports::sync::SyncEvent;
use domain::sync::{DeviceKey, PairingCode};

use super::support::{contents, device, eventually, space, write_note};
use crate::{
    Error, Fingerprint, MAX_FAILURES, MemoryNetwork, OfferChoice, PairingState, PairingUri,
};

fn wrong(code: &PairingCode) -> PairingCode {
    let number: u32 = code.as_str().parse().unwrap();
    PairingCode::from_number((number + 1) % 1_000_000).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_right_code_links_both_devices_and_syncs_right_after_acceptance() {
    let network = MemoryNetwork::new();
    let (a, b) = (device(&network, "Bureau"), device(&network, "Portable"));
    let work = a.create_space("travail", space(1));
    write_note(&work, "compte rendu");
    write_note(&work, "liste 🥚 été");

    let ticket = a
        .engine
        .start_pairing(vec!["travail".to_owned()])
        .await
        .unwrap();
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

    eventually("offre reçue", || {
        b.has_event(|event| {
            matches!(event, SyncEvent::OfferReceived { offers, .. }
                if offers.len() == 1
                    && offers[0].name == "Profil travail"
                    && offers[0].note_count == 2)
        })
    })
    .await;
    b.engine
        .answer_offer(a.key(), space(1), OfferChoice::NewProfile)
        .await
        .unwrap();

    eventually("synchronisation initiale", || {
        b.has_event(|event| matches!(event, SyncEvent::InitialSyncDone { .. }))
    })
    .await;
    let joined = b.store("p1");
    assert_eq!(contents(&joined), contents(&work));
    assert!(b.has_event(|event| matches!(
        event,
        SyncEvent::Progress {
            done: 3,
            total: 3,
            ..
        }
    )));

    // Ensuite, le temps réel : une modification de B arrive chez A
    write_note(&joined, "écrit sur le portable");
    b.engine.local_change(&"p1".to_owned());
    eventually("temps réel", || contents(&work).len() == 3).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_wrong_code_is_refused_on_both_sides() {
    let network = MemoryNetwork::new();
    let (a, b) = (device(&network, "Bureau"), device(&network, "Portable"));
    a.create_space("travail", space(1));
    let ticket = a
        .engine
        .start_pairing(vec!["travail".to_owned()])
        .await
        .unwrap();

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
    let ticket1 = a1.engine.start_pairing(vec!["p".to_owned()]).await.unwrap();
    let ticket2 = a2.engine.start_pairing(vec!["p".to_owned()]).await.unwrap();

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
    let first = a.engine.start_pairing(vec!["p".to_owned()]).await.unwrap();
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
    let ticket = a.engine.start_pairing(vec!["p".to_owned()]).await.unwrap();
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
    let ticket = a.engine.start_pairing(vec!["p".to_owned()]).await.unwrap();

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
