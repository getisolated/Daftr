//! Collections partagées de bout en bout : deux installations, de vraies bases SQLite et de
//! vrais dossiers surveillés. B n'a rien en commun avec A que la confiance : la collection lui
//! arrive comme une entrée « à configurer », rien n'est écrit avant son activation dans le
//! profil choisi (même inactif), puis les fichiers arrivent et les modifications faites sur le
//! disque (surveillance du système) suivent.

use std::path::Path;

use app::ports::CollectionStore;
use app::ports::collection_sync::CollectionSyncStore;
use app::ports::sync::SyncEvent;
use domain::collection::CollectionKind;
use domain::profile::ProfileId;
use platform::paths::canonical_path;
use presentation::collection_sync::OfferMode;
use presentation::dto::CollectionShareStatusKind;

use super::end_to_end::{link_by_code, pair_only};
use super::installation::{Installation, eventually};

fn write(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn read(root: &Path, relative: &str) -> Option<String> {
    std::fs::read_to_string(root.join(relative)).ok()
}

fn offer_received(install: &Installation) -> bool {
    install
        .events
        .has(|event| matches!(event, SyncEvent::CollectionOfferReceived { .. }))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_collection_is_offered_activated_in_an_inactive_profile_and_followed_on_disk() {
    let (a, b) = (Installation::new("A"), Installation::new("B"));
    let shared = ProfileId::default_profile();
    let a_store = a.activate(&shared);
    // Sur B, le profil actif n'est pas celui qui reçoit la collection
    let b_active = b.create_profile("Actif");
    b.activate(&b_active);
    let b_target = b.create_profile("Perso");
    for install in [&a, &b] {
        install.sync.start(&install.settings).await;
    }
    pair_only(&a, &b).await;

    let source = tempfile::tempdir().unwrap();
    let root = canonical_path(source.path());
    write(&root, "README.md", "# Docs");
    write(&root, "guide/intro.md", "intro");
    write(&root, "image.png", "not markdown");
    let root_text = root.to_string_lossy().into_owned();
    a_store
        .add(&root_text, CollectionKind::Folder, "Docs")
        .unwrap();

    let summary = a.sync.folder_summary(root.clone()).await;
    assert_eq!((summary.md_files, summary.has_git), (2, false));
    let offered = a
        .sync
        .share_collection(
            shared.clone(),
            root_text.clone(),
            vec![b.sync.device_key()],
            &a.settings.current(),
        )
        .await
        .unwrap();
    assert_eq!(offered.status, CollectionShareStatusKind::Offered);

    eventually("proposition reçue", || offer_received(&b)).await;
    let state = b
        .sync
        .collection_state(None, &b.settings.current())
        .await
        .unwrap();
    assert_eq!(state.offers.len(), 1);
    let offer = &state.offers[0];
    assert_eq!((offer.name.as_str(), offer.files), ("Docs", 2));
    assert_eq!(offer.source_profile_id, "default");
    // Le profil source n'est pas synchronisé vers B : le profil cible est à choisir
    assert_eq!(offer.source_profile_synced_here, None);
    assert!(Path::new(&offer.suggested_destination).starts_with(&state.received_folder));
    // Les profils de B sont tous listés, aucun n'a encore de partage
    assert!(state.shares.is_empty());
    let received = tempfile::tempdir().unwrap();
    let destination = canonical_path(received.path()).join("Docs");
    assert!(!destination.exists(), "rien n'est écrit avant l'activation");

    let share = domain::uuid::Uuid::parse(&offered.sync_id).unwrap();
    b.sync
        .activate_collection(
            a.sync.device_key(),
            share,
            b_target.clone(),
            destination.clone(),
            OfferMode::Receive,
        )
        .await
        .unwrap();

    eventually("fichiers reçus", || {
        read(&destination, "guide/intro.md").is_some()
    })
    .await;
    assert_eq!(read(&destination, "README.md").as_deref(), Some("# Docs"));
    assert!(read(&destination, "image.png").is_none());
    // La collection est dans le profil choisi, pas dans le profil actif
    let record = b
        .sync
        .profiles
        .with_sqlite(&b_target, |store| store.share(&share))
        .unwrap()
        .unwrap();
    assert_eq!(record.root, destination.to_string_lossy());
    let in_active = b
        .sync
        .profiles
        .with_sqlite(&b_active, |store| store.shares())
        .unwrap();
    assert!(in_active.is_empty());
    let after = b
        .sync
        .collection_state(Some(b_target.clone()), &b.settings.current())
        .await
        .unwrap();
    assert!(after.offers.is_empty());
    assert_eq!(after.collections.len(), 1);
    assert_eq!(
        after.collections[0].sync_id.as_deref(),
        Some(&*offered.sync_id)
    );
    assert_eq!(after.shares.len(), 1);

    write(&root, "README.md", "# Docs v2");
    eventually("modification suivie par la surveillance", || {
        read(&destination, "README.md").as_deref() == Some("# Docs v2")
    })
    .await;
    write(&destination, "from-b.md", "hello");
    eventually("fichier de B reçu par A", || {
        read(&root, "from-b.md").as_deref() == Some("hello")
    })
    .await;
    assert!(
        a.events
            .has(|event| matches!(event, SyncEvent::CollectionChanged { .. }))
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let after = a
            .sync
            .collection_state(None, &a.settings.current())
            .await
            .unwrap();
        assert_eq!(after.shares.len(), 1);
        if after.shares[0].files == 3 {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "résumé du dossier non à jour"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_target_profile_is_preselected_when_the_source_profile_is_synced_here() {
    let (a, b) = (Installation::new("A"), Installation::new("B"));
    let shared = ProfileId::default_profile();
    let a_store = a.activate(&shared);
    b.activate(&b.create_profile("Perso"));
    for install in [&a, &b] {
        install.sync.start(&install.settings).await;
    }
    let b_profile = link_by_code(&a, &shared, &b).await;
    let source = tempfile::tempdir().unwrap();
    let root = canonical_path(source.path());
    write(&root, "a.md", "from a");
    let root_text = root.to_string_lossy().into_owned();
    a_store
        .add(&root_text, CollectionKind::Folder, "Docs")
        .unwrap();

    a.sync
        .share_collection(
            shared,
            root_text,
            vec![b.sync.device_key()],
            &a.settings.current(),
        )
        .await
        .unwrap();

    eventually("proposition reçue", || offer_received(&b)).await;
    let state = b
        .sync
        .collection_state(None, &b.settings.current())
        .await
        .unwrap();
    assert_eq!(
        state.offers[0].source_profile_synced_here.as_deref(),
        Some(b_profile.as_str())
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_folder_with_files_needs_the_merge_and_an_ignored_offer_leaves_no_trace() {
    let (a, b) = (Installation::new("A"), Installation::new("B"));
    let shared = ProfileId::default_profile();
    let a_store = a.activate(&shared);
    let b_profile = b.create_profile("Perso");
    b.activate(&b_profile);
    for install in [&a, &b] {
        install.sync.start(&install.settings).await;
    }
    pair_only(&a, &b).await;
    let source = tempfile::tempdir().unwrap();
    let root = canonical_path(source.path());
    write(&root, "a.md", "from a");
    let root_text = root.to_string_lossy().into_owned();
    a_store
        .add(&root_text, CollectionKind::Folder, "Docs")
        .unwrap();
    let offered = a
        .sync
        .share_collection(
            shared,
            root_text,
            vec![b.sync.device_key()],
            &a.settings.current(),
        )
        .await
        .unwrap();
    let share = domain::uuid::Uuid::parse(&offered.sync_id).unwrap();
    eventually("proposition reçue", || offer_received(&b)).await;
    let existing = tempfile::tempdir().unwrap();
    let destination = canonical_path(existing.path());
    write(&destination, "mine.md", "from b");

    let refused = b
        .sync
        .activate_collection(
            a.sync.device_key(),
            share,
            b_profile.clone(),
            destination.clone(),
            OfferMode::Receive,
        )
        .await;
    assert!(refused.is_err(), "un dossier non vide demande la fusion");
    assert!(read(&destination, "a.md").is_none());

    b.sync
        .ignore_collection(a.sync.device_key(), share)
        .await
        .unwrap();

    assert!(
        b.sync
            .collection_state(None, &b.settings.current())
            .await
            .unwrap()
            .offers
            .is_empty()
    );
    let shares = b
        .sync
        .profiles
        .with_sqlite(&b_profile, |store| store.shares())
        .unwrap();
    assert!(shares.is_empty());
    assert_eq!(read(&destination, "mine.md").as_deref(), Some("from b"));
    assert!(read(&destination, "a.md").is_none());
    // Le refus est revenu chez A : plus de partage
    eventually("partage retiré chez A", || {
        a_store.share(&share).unwrap().is_none()
    })
    .await;
}
