//! De bout en bout, sur loopback et de vraies bases SQLite : A s'appaire avec B par code, la
//! synchronisation complète suit aussitôt, une modification sur A arrive sur B dont le profil
//! synchronisé n'est pas actif, puis C, lié au seul B, reçoit les modifications de A par B.

use app::ports::NoteStore;
use app::ports::sync::{SyncEvent, SyncProfiles};
use domain::note::{NewNote, NoteChanges};
use domain::profile::ProfileId;
use domain::sync::{DeviceKey, PairingCode, SpaceId};
use presentation::sync::OfferAnswer;

use super::installation::{Installation, eventually};

/// `joiner` se lie par code à `host`, qui propose `profile` ; il accepte en nouveau profil et
/// attend la fin de la synchronisation initiale. Rend son profil.
async fn link_by_code(
    host: &Installation,
    profile: &ProfileId,
    joiner: &Installation,
) -> ProfileId {
    let settings = host.settings.current();
    let ticket = host
        .sync
        .start_pairing(vec![profile.clone()], &settings)
        .await
        .unwrap();
    let code = PairingCode::parse(&ticket.code).unwrap();

    let linked = joiner.sync.pair(code, Some(host.addr())).await.unwrap();
    assert_eq!(linked.key, host.sync.device_key().to_string());

    eventually("offre reçue", || joiner.events.find(offer).is_some()).await;
    let (device, space) = joiner.events.find(offer).unwrap();
    joiner
        .sync
        .answer_offer(device, space, OfferAnswer::NewProfile)
        .await
        .unwrap();
    eventually("synchronisation initiale", || {
        joiner
            .events
            .has(|event| matches!(event, SyncEvent::InitialSyncDone { .. }))
    })
    .await;
    joiner.sync.profiles.profile_for(&space).unwrap().unwrap()
}

fn offer(event: &SyncEvent<ProfileId>) -> Option<(DeviceKey, SpaceId)> {
    match event {
        SyncEvent::OfferReceived { device, offers, .. } => {
            offers.first().map(|offer| (*device, offer.space_id))
        }
        _ => None,
    }
}

fn edit(content: &str) -> NoteChanges {
    NoteChanges {
        content: Some(content.to_owned()),
        ..NoteChanges::default()
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn paired_devices_sync_at_once_then_relay_changes() {
    let (a, b, c) = (
        Installation::new("A"),
        Installation::new("B"),
        Installation::new("C"),
    );
    let shared = ProfileId::default_profile();
    let a_store = a.activate(&shared);
    let note = a_store
        .insert(&NewNote {
            title: "Courses".to_owned(),
            content: "Courses\n- pain".to_owned(),
            tags: vec![],
            folder_id: None,
        })
        .unwrap();
    // Sur B, un autre profil est actif : le profil reçu tourne en arrière-plan
    let b_other = b.create_profile("Perso");
    b.activate(&b_other);
    c.activate(&ProfileId::default_profile());
    for install in [&a, &b, &c] {
        install.sync.start(&install.settings).await;
    }

    let b_profile = link_by_code(&a, &shared, &b).await;
    assert_eq!(b.contents(&b_profile), ["Courses\n- pain"]);
    assert_ne!(b_profile, b_other);

    a_store
        .update(note.id, &edit("Courses\n- pain\n- lait"))
        .unwrap();
    eventually("modification de A reçue par B", || {
        b.contents(&b_profile) == ["Courses\n- pain\n- lait"]
    })
    .await;

    let c_profile = link_by_code(&b, &b_profile, &c).await;
    assert_eq!(c.contents(&c_profile), ["Courses\n- pain\n- lait"]);
    assert!(
        !a.sync
            .engine
            .connected_devices()
            .contains(&c.sync.device_key())
    );

    a_store
        .update(note.id, &edit("Courses\n- pain\n- lait\n- œufs"))
        .unwrap();
    eventually("modification de A relayée par B jusqu'à C", || {
        c.contents(&c_profile) == ["Courses\n- pain\n- lait\n- œufs"]
    })
    .await;
    assert!(b.events.has(|event| matches!(
        event,
        SyncEvent::NotesSynced { profile, .. } if *profile == b_profile
    )));
}
