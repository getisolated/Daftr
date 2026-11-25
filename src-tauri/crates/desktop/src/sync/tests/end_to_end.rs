//! De bout en bout, sur loopback et de vraies bases SQLite : A s'appaire avec B par code, puis
//! partage un profil avec lui ; B en crée un clone sans rien confirmer et la synchronisation
//! complète suit. Une modification sur A arrive sur B dont le profil synchronisé n'est pas actif,
//! puis C, lié au seul B, reçoit les modifications de A par B.

use app::ports::NoteStore;
use app::ports::sync::SyncEvent;
use domain::note::{NewNote, NoteChanges};
use domain::profile::ProfileId;
use domain::sync::PairingCode;

use super::installation::{Installation, eventually};

/// `joiner` se lie par code à `host`, sans aucun profil : chacun fait ensuite confiance à
/// l'autre.
pub(super) async fn pair_only(host: &Installation, joiner: &Installation) {
    let ticket = host.sync.start_pairing().await.unwrap();
    let code = PairingCode::parse(&ticket.code).unwrap();

    let linked = joiner.sync.pair(code, Some(host.addr())).await.unwrap();
    assert_eq!(linked.key, host.sync.device_key().to_string());
    let joiner_key = joiner.sync.device_key();
    eventually("liaison publiée", || {
        host.events.has(|event| {
            matches!(event, SyncEvent::DeviceLinked { device, .. } if *device == joiner_key)
        })
    })
    .await;
}

/// `joiner` se lie par code à `host`, qui partage `profile` avec lui ; le clone est créé sans
/// rien demander, puis la synchronisation initiale se termine. Rend le profil cloné.
pub(super) async fn link_by_code(
    host: &Installation,
    profile: &ProfileId,
    joiner: &Installation,
) -> ProfileId {
    pair_only(host, joiner).await;
    let shares = host
        .sync
        .set_profile_shared(
            profile.clone(),
            joiner.sync.device_key(),
            true,
            &host.settings.current(),
        )
        .await
        .unwrap();
    let row = shares
        .profiles
        .iter()
        .find(|row| row.profile_id == profile.as_str())
        .unwrap();
    assert!(row.devices[0].shared);

    eventually("profil reçu", || joiner.events.find(received).is_some()).await;
    eventually("synchronisation initiale", || {
        joiner
            .events
            .has(|event| matches!(event, SyncEvent::InitialSyncDone { .. }))
    })
    .await;
    joiner.events.find(received).unwrap()
}

fn received(event: &SyncEvent<ProfileId>) -> Option<ProfileId> {
    match event {
        SyncEvent::ProfileReceived { profile, .. } => Some(profile.clone()),
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
    // Le profil par défaut de A est fusionné avec celui de B, qui garde son nom (aucun clone)
    assert!(b_profile.is_default());
    let merged = b.profiles.get(&b_profile).unwrap();
    assert_eq!(merged.name, None);
    assert!(!merged.is_protected());

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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_shared_profile_arrives_with_its_name_and_icon_and_no_password() {
    let (a, b) = (Installation::new("A"), Installation::new("B"));
    a.activate(&ProfileId::default_profile());
    b.activate(&ProfileId::default_profile());
    let work = a.create_profile("Travail");
    let icon = b"\x89PNG\r\n\x1a\n-travail-";
    a.profiles.set_icon(&work, icon).unwrap();
    a.profiles
        .set_password(&work, None, Some("secret"))
        .unwrap();
    for install in [&a, &b] {
        install.sync.start(&install.settings).await;
    }

    let cloned = link_by_code(&a, &work, &b).await;

    let profile = b.profiles.get(&cloned).unwrap();
    assert_eq!(profile.name.as_deref(), Some("Travail"));
    assert!(
        !profile.is_protected(),
        "le mot de passe maître n'est pas transmis"
    );
    let (format, bytes) = b.profiles.icon(&cloned).unwrap().unwrap();
    assert_eq!(format, domain::profile::IconFormat::Png);
    assert_eq!(bytes, icon);
    // La source garde son mot de passe
    assert!(a.profiles.get(&work).unwrap().is_protected());
}
