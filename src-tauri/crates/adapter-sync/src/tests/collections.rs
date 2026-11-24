//! Collections partagées entre deux moteurs réels sur loopback : proposition, acceptation,
//! échange des fichiers, suppressions, conflits, fusion avec un dossier existant, refus.

use std::path::{Path, PathBuf};

use app::collection_sync::{IgnoredReason, MAX_FILE_BYTES};
use app::ports::collection_sync::{
    CollectionDisk, CollectionInbox, CollectionSyncStore, PendingCollection,
};
use app::ports::sync::SyncEvent;
use domain::uuid::Uuid;

use super::support::{Device, device, eventually, link, share, space};
use crate::MemoryNetwork;

/// Le profil de A qui porte la collection.
const PROFILE: &str = "p";
/// Le profil de B qui la reçoit : il ne partage rien avec A.
const TARGET: &str = "q";

/// A et B se font confiance sans partager de profil ; A a la collection `docs` dans `p`, B
/// a un profil `q` pour la recevoir.
struct Pair {
    a: Device,
    b: Device,
    a_root: PathBuf,
    b_root: PathBuf,
    // Garde le réseau en mémoire vivant
    _network: std::sync::Arc<MemoryNetwork>,
}

fn root(name: &str) -> PathBuf {
    std::env::temp_dir().join("daftr-sync-tests").join(name)
}

async fn pair() -> Pair {
    let network = MemoryNetwork::new();
    let (a, b) = (device(&network, "A"), device(&network, "B"));
    a.profiles.add(PROFILE);
    b.profiles.add(TARGET);
    link(&a, &b);
    let a_root = root("a-docs");
    a.store(PROFILE)
        .list_collection(&a_root.to_string_lossy(), "Docs");
    a.disk.create_directory(&a_root);
    Pair {
        a,
        b,
        a_root,
        b_root: root("b-docs"),
        _network: network,
    }
}

impl Pair {
    async fn start(&self) {
        self.a.engine.refresh().await.unwrap();
        self.b.engine.refresh().await.unwrap();
    }

    /// A propose la collection à B ; rend l'identifiant du partage.
    async fn offer(&self) -> Uuid {
        let root = self.a_root.to_string_lossy().into_owned();
        let record = self
            .a
            .engine
            .share_collection(PROFILE.to_owned(), root, vec![self.b.key()])
            .await
            .unwrap();
        record.sync_id
    }

    /// A propose une seconde collection à B ; rend l'identifiant du partage.
    async fn offer_second(&self) -> Uuid {
        let second = root("a-second");
        self.a
            .store(PROFILE)
            .list_collection(&second.to_string_lossy(), "Second");
        self.a.disk.create_directory(&second);
        self.a
            .engine
            .share_collection(
                PROFILE.to_owned(),
                second.to_string_lossy().into_owned(),
                vec![self.b.key()],
            )
            .await
            .unwrap()
            .sync_id
    }

    async fn accept(&self, share: Uuid, merge: bool) {
        eventually("proposition reçue", || !self.pending().is_empty()).await;
        self.b
            .engine
            .activate_collection(
                self.a.key(),
                share,
                TARGET.to_owned(),
                self.b_root.clone(),
                merge,
            )
            .await
            .unwrap();
    }

    /// Les entrées « à configurer » de B.
    fn pending(&self) -> Vec<PendingCollection> {
        let mut entries = self.b.inbox.pending().unwrap();
        entries.retain(|entry| !entry.ignored);
        entries
    }

    async fn a_changed(&self, share: Uuid) {
        self.a
            .engine
            .collection_changed(PROFILE.to_owned(), share)
            .await
            .unwrap();
    }

    fn a_files(&self) -> Vec<String> {
        self.a.disk.paths(&self.a_root)
    }

    fn b_files(&self) -> Vec<String> {
        self.b.disk.paths(&self.b_root)
    }
}

fn text(device: &Device, root: &Path, path: &str) -> Option<String> {
    device.disk.text(root, path)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn nothing_is_written_before_the_user_accepts_then_files_arrive() {
    let pair = pair().await;
    pair.a.disk.put(&pair.a_root, "README.md", b"# Docs");
    pair.a.disk.put(&pair.a_root, "guide/intro.md", b"intro");
    pair.start().await;

    let share = pair.offer().await;
    eventually("proposition reçue", || {
        pair.b.has_event(|event| {
            matches!(event, SyncEvent::CollectionOfferReceived { offers, .. }
                if offers[0].files == 2 && offers[0].name == "Docs"
                    && offers[0].source_profile_id == PROFILE)
        })
    })
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert!(pair.b_files().is_empty(), "rien avant l'acceptation");
    assert!(pair.b.store(TARGET).shares().unwrap().is_empty());
    assert!(!pair.b.disk.is_directory(&pair.b_root));

    pair.accept(share, false).await;

    eventually("fichiers reçus", || {
        pair.b_files() == ["README.md", "guide/intro.md"]
    })
    .await;
    assert_eq!(
        text(&pair.b, &pair.b_root, "guide/intro.md").as_deref(),
        Some("intro")
    );
    let record = pair.b.store(TARGET).share(&share).unwrap().unwrap();
    assert_eq!(record.root, pair.b_root.to_string_lossy());
    assert!(record.is_active_with(&pair.a.key()));
    eventually("côté A actif", || {
        pair.a
            .store(PROFILE)
            .share(&share)
            .unwrap()
            .unwrap()
            .is_active_with(&pair.b.key())
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn changes_deletions_and_new_files_follow_in_both_directions() {
    let pair = pair().await;
    pair.a.disk.put(&pair.a_root, "a.md", b"one");
    pair.a.disk.put(&pair.a_root, "gone.md", b"bye");
    pair.start().await;
    let share = pair.offer().await;
    pair.accept(share, false).await;
    eventually("copie initiale", || pair.b_files() == ["a.md", "gone.md"]).await;
    eventually("bases posées", || {
        pair.a
            .store(PROFILE)
            .bases(&share, &pair.b.key())
            .unwrap()
            .len()
            == 2
    })
    .await;

    pair.a.disk.put(&pair.a_root, "a.md", b"two");
    pair.a.disk.remove(&pair.a_root, "gone.md");
    pair.a_changed(share).await;
    eventually("modification et suppression reçues", || {
        text(&pair.b, &pair.b_root, "a.md").as_deref() == Some("two") && pair.b_files() == ["a.md"]
    })
    .await;
    assert_eq!(pair.b.disk.trashed(&pair.b_root), ["gone.md"]);

    pair.b.disk.put(&pair.b_root, "from-b.md", b"hello");
    pair.b
        .engine
        .collection_changed(TARGET.to_owned(), share)
        .await
        .unwrap();
    eventually("fichier de B reçu par A", || {
        text(&pair.a, &pair.a_root, "from-b.md").as_deref() == Some("hello")
    })
    .await;
    // Plus rien à échanger : les deux copies sont identiques
    assert_eq!(pair.a_files(), pair.b_files());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_folder_that_is_not_empty_needs_the_merge() {
    let pair = pair().await;
    pair.a.disk.put(&pair.a_root, "a.md", b"a");
    pair.b.disk.put(&pair.b_root, "existing.md", b"b");
    pair.start().await;
    let share = pair.offer().await;
    eventually("proposition reçue", || !pair.pending().is_empty()).await;

    let refused = pair
        .b
        .engine
        .activate_collection(
            pair.a.key(),
            share,
            TARGET.to_owned(),
            pair.b_root.clone(),
            false,
        )
        .await;

    assert!(refused.is_err());
    assert_eq!(pair.b_files(), ["existing.md"]);
    assert!(pair.b.store(TARGET).shares().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn merging_keeps_identical_files_copies_the_missing_ones_and_keeps_both_versions() {
    let pair = pair().await;
    pair.a.disk.put(&pair.a_root, "same.md", b"identical");
    pair.a.disk.put(&pair.a_root, "only-a.md", b"a");
    pair.a.disk.put(&pair.a_root, "differs.md", b"version A");
    pair.b.disk.put(&pair.b_root, "same.md", b"identical");
    pair.b.disk.put(&pair.b_root, "only-b.md", b"b");
    pair.b.disk.put(&pair.b_root, "differs.md", b"version B");
    pair.start().await;
    let share = pair.offer().await;

    pair.accept(share, true).await;

    eventually("deux dossiers identiques", || {
        pair.a_files().len() == 5 && pair.a_files() == pair.b_files()
    })
    .await;
    let files = pair.a_files();
    let copy = files
        .iter()
        .find(|name| name.contains("(conflit"))
        .unwrap()
        .clone();
    // Les deux versions sont gardées, chacune des deux copies avec le même contenu
    let versions = [
        text(&pair.a, &pair.a_root, "differs.md"),
        text(&pair.a, &pair.a_root, &copy),
    ];
    let mut versions: Vec<String> = versions.into_iter().flatten().collect();
    versions.sort();
    assert_eq!(versions, ["version A", "version B"]);
    for name in &files {
        assert_eq!(
            text(&pair.a, &pair.a_root, name),
            text(&pair.b, &pair.b_root, name),
            "{name}"
        );
    }
    assert_eq!(
        text(&pair.b, &pair.b_root, "same.md").as_deref(),
        Some("identical")
    );
    assert!(
        pair.b.disk.trashed(&pair.b_root).is_empty(),
        "rien n'est supprimé"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn disjoint_edits_made_on_both_sides_are_merged() {
    let pair = pair().await;
    pair.a.disk.put(&pair.a_root, "n.md", b"un\ndeux\ntrois\n");
    pair.start().await;
    let share = pair.offer().await;
    pair.accept(share, false).await;
    eventually("copie initiale", || pair.b_files() == ["n.md"]).await;
    eventually("bases posées avec le texte", || {
        let store = pair.b.store(TARGET);
        store
            .base(&share, &pair.a.key(), "n.md")
            .unwrap()
            .is_some_and(|base| base.text.is_some())
    })
    .await;
    // Les deux côtés sont hors ligne l'un pour l'autre pendant les éditions
    pair.a.engine.set_paused(true).await.unwrap();
    pair.b.engine.set_paused(true).await.unwrap();
    pair.a.disk.put(&pair.a_root, "n.md", b"UN\ndeux\ntrois\n");
    pair.b
        .disk
        .put(&pair.b_root, "n.md", b"un\ndeux\ntrois\nquatre\n");

    pair.a.engine.set_paused(false).await.unwrap();
    pair.b.engine.set_paused(false).await.unwrap();

    eventually("fusion identique des deux côtés", || {
        let merged = Some("UN\ndeux\ntrois\nquatre\n".to_owned());
        text(&pair.a, &pair.a_root, "n.md") == merged
            && text(&pair.b, &pair.b_root, "n.md") == merged
    })
    .await;
    assert_eq!(pair.a_files(), ["n.md"], "aucune copie de conflit");
    assert_eq!(pair.b_files(), ["n.md"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_modification_wins_over_a_deletion_made_while_apart() {
    let pair = pair().await;
    pair.a.disk.put(&pair.a_root, "n.md", b"v1");
    pair.start().await;
    let share = pair.offer().await;
    pair.accept(share, false).await;
    eventually("copie initiale", || pair.b_files() == ["n.md"]).await;
    eventually("bases posées", || {
        pair.a
            .store(PROFILE)
            .bases(&share, &pair.b.key())
            .unwrap()
            .len()
            == 1
            && pair
                .b
                .store(TARGET)
                .bases(&share, &pair.a.key())
                .unwrap()
                .len()
                == 1
    })
    .await;
    pair.a.engine.set_paused(true).await.unwrap();
    pair.b.engine.set_paused(true).await.unwrap();
    pair.a.disk.remove(&pair.a_root, "n.md");
    pair.b.disk.put(&pair.b_root, "n.md", b"v2");

    pair.a.engine.set_paused(false).await.unwrap();
    pair.b.engine.set_paused(false).await.unwrap();

    eventually("le fichier modifié est rétabli chez A", || {
        text(&pair.a, &pair.a_root, "n.md").as_deref() == Some("v2")
    })
    .await;
    assert_eq!(text(&pair.b, &pair.b_root, "n.md").as_deref(), Some("v2"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn changes_made_while_disconnected_are_exchanged_on_reconnection() {
    let pair = pair().await;
    pair.a.disk.put(&pair.a_root, "a.md", b"one");
    pair.start().await;
    let share = pair.offer().await;
    pair.accept(share, false).await;
    eventually("copie initiale", || pair.b_files() == ["a.md"]).await;
    pair.b.engine.set_paused(true).await.unwrap();
    pair.a.disk.put(&pair.a_root, "later.md", b"while apart");

    pair.b.engine.set_paused(false).await.unwrap();

    eventually("rattrapage", || {
        text(&pair.b, &pair.b_root, "later.md").as_deref() == Some("while apart")
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_failed_write_is_retried_by_the_next_manifest() {
    let pair = pair().await;
    for index in 0..5 {
        pair.a.disk.put(&pair.a_root, &format!("f{index}.md"), b"x");
    }
    pair.b.disk.fail_writes(true);
    pair.start().await;
    let share = pair.offer().await;
    pair.accept(share, false).await;
    eventually("échange terminé sans écriture", || {
        pair.b
            .engine
            .collection_status(&share, &[pair.a.key()])
            .peers[0]
            .total
            == 5
            && !pair
                .b
                .engine
                .collection_status(&share, &[pair.a.key()])
                .peers[0]
                .syncing
    })
    .await;
    assert!(pair.b_files().is_empty());

    pair.b.disk.fail_writes(false);
    pair.a.disk.put(&pair.a_root, "kick.md", b"y");
    pair.a_changed(share).await;

    eventually("reprise au premier fichier manquant", || {
        pair.b_files().len() == 6
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn declining_clears_the_offer_and_blocks_nothing() {
    let pair = pair().await;
    pair.a.disk.put(&pair.a_root, "a.md", b"a");
    pair.start().await;
    let share = pair.offer().await;
    eventually("proposition reçue", || !pair.pending().is_empty()).await;

    pair.b
        .engine
        .ignore_collection(pair.a.key(), share)
        .await
        .unwrap();

    eventually("partage retiré chez A", || {
        pair.a.store(PROFILE).share(&share).unwrap().is_none()
    })
    .await;
    assert!(pair.pending().is_empty());
    assert!(pair.b_files().is_empty());
    // Une nouvelle proposition reste possible
    let again = pair.offer().await;
    eventually("nouvelle proposition", || !pair.pending().is_empty()).await;
    assert_ne!(again, share);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stopping_cuts_the_link_on_both_sides_and_keeps_the_files() {
    let pair = pair().await;
    pair.a.disk.put(&pair.a_root, "a.md", b"a");
    pair.start().await;
    let share = pair.offer().await;
    pair.accept(share, false).await;
    eventually("copie initiale", || pair.b_files() == ["a.md"]).await;

    pair.a
        .engine
        .stop_collection(PROFILE.to_owned(), share)
        .await
        .unwrap();

    eventually("lien coupé chez B", || {
        pair.b.store(TARGET).share(&share).unwrap().is_none()
    })
    .await;
    assert_eq!(pair.a_files(), ["a.md"]);
    assert_eq!(pair.b_files(), ["a.md"]);
    pair.a.disk.put(&pair.a_root, "after.md", b"x");
    pair.a
        .engine
        .collection_changed(PROFILE.to_owned(), share)
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert_eq!(pair.b_files(), ["a.md"], "plus rien n'est échangé");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn only_trusted_devices_can_receive_a_collection() {
    let pair = pair().await;
    let outsider = device(&pair._network, "C");
    pair.a.disk.put(&pair.a_root, "a.md", b"a");
    let root = pair.a_root.to_string_lossy().into_owned();

    let refused = pair
        .a
        .engine
        .share_collection(PROFILE.to_owned(), root.clone(), vec![outsider.key()])
        .await;
    let nobody = pair
        .a
        .engine
        .share_collection(PROFILE.to_owned(), root.clone(), vec![])
        .await;
    // B ne partage aucun profil avec A : la confiance suffit
    let accepted = pair
        .a
        .engine
        .share_collection(PROFILE.to_owned(), root, vec![pair.b.key()])
        .await;

    assert!(refused.is_err() && nobody.is_err());
    assert!(accepted.is_ok());
    assert_eq!(pair.a.store(PROFILE).shares().unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_offer_is_a_persisted_entry_to_configure_that_names_the_source_profile() {
    let pair = pair().await;
    pair.a.disk.put(&pair.a_root, "a.md", b"a");
    pair.a.profiles.set_name(PROFILE, "Travail");
    pair.start().await;
    let share = pair.offer().await;

    eventually("entrée reçue", || !pair.pending().is_empty()).await;

    let entry = &pair.pending()[0];
    assert_eq!(entry.device, pair.a.key());
    assert_eq!(entry.offer.share, share);
    assert_eq!(entry.offer.source_profile_id, PROFILE);
    assert_eq!(entry.offer.source_profile_name, "Travail");
    // Le profil source n'est pas synchronisé vers B : aucune présélection
    assert_eq!(entry.offer.source_space, None);
    // Rien n'est écrit ni enregistré avant l'activation
    assert!(pair.b.store(TARGET).shares().unwrap().is_empty());
    assert!(pair.b.store(TARGET).listed_collections().is_empty());
    assert!(!pair.b.disk.is_directory(&pair.b_root));
    assert_eq!(pair.b.engine.pending_collections().await.unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_source_space_is_named_when_the_profile_is_synced_with_the_device() {
    let pair = pair().await;
    pair.a
        .store(PROFILE)
        .create_space(space(7), "Profil p", &pair.a.name, super::support::NOW)
        .unwrap();
    share(&pair.a, PROFILE, &pair.b, "copie");
    pair.a.disk.put(&pair.a_root, "a.md", b"a");
    pair.start().await;
    pair.offer().await;

    eventually("entrée reçue", || !pair.pending().is_empty()).await;

    assert_eq!(pair.pending()[0].offer.source_space, Some(space(7)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_entry_to_configure_survives_a_restart() {
    let pair = pair().await;
    pair.a.disk.put(&pair.a_root, "a.md", b"a");
    pair.start().await;
    let share = pair.offer().await;
    eventually("entrée reçue", || !pair.pending().is_empty()).await;

    // B redémarre : le moteur est neuf, la liste est dans le fichier
    pair.b.engine.shutdown().await;
    pair.b.engine.refresh().await.unwrap();

    assert_eq!(pair.pending().len(), 1);
    pair.accept(share, false).await;
    eventually("fichiers reçus", || pair.b_files() == ["a.md"]).await;
    assert!(pair.pending().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_collection_goes_into_the_chosen_profile_whatever_the_source_one() {
    let pair = pair().await;
    pair.b.profiles.add("autre");
    pair.a.disk.put(&pair.a_root, "a.md", b"a");
    pair.start().await;
    let share = pair.offer().await;
    eventually("entrée reçue", || !pair.pending().is_empty()).await;

    pair.b
        .engine
        .activate_collection(
            pair.a.key(),
            share,
            "autre".to_owned(),
            pair.b_root.clone(),
            false,
        )
        .await
        .unwrap();

    eventually("fichiers reçus", || pair.b_files() == ["a.md"]).await;
    let record = pair.b.store("autre").share(&share).unwrap().unwrap();
    assert_eq!(record.root, pair.b_root.to_string_lossy());
    assert!(pair.b.store(TARGET).shares().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_ignored_entry_is_refused_to_the_device_even_when_it_was_absent() {
    let pair = pair().await;
    pair.a.disk.put(&pair.a_root, "a.md", b"a");
    pair.start().await;
    let share = pair.offer().await;
    eventually("entrée reçue", || !pair.pending().is_empty()).await;
    pair.a.engine.set_paused(true).await.unwrap();
    pair.b.engine.set_paused(true).await.unwrap();

    pair.b
        .engine
        .ignore_collection(pair.a.key(), share)
        .await
        .unwrap();

    // Hors connexion : l'entrée disparaît de la liste mais le refus reste à envoyer
    assert!(pair.pending().is_empty());
    assert!(pair.b.inbox.pending().unwrap()[0].ignored);
    pair.a.engine.set_paused(false).await.unwrap();
    pair.b.engine.set_paused(false).await.unwrap();
    eventually("partage retiré chez A", || {
        pair.a.store(PROFILE).share(&share).unwrap().is_none()
    })
    .await;
    eventually("entrée oubliée chez B", || {
        pair.b.inbox.pending().unwrap().is_empty()
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn unsharing_with_one_device_removes_its_entry_and_keeps_the_others() {
    let pair = pair().await;
    let c = device(&pair._network, "C");
    link(&pair.a, &c);
    c.profiles.add("c");
    pair.a.disk.put(&pair.a_root, "a.md", b"a");
    pair.start().await;
    c.engine.refresh().await.unwrap();
    let root = pair.a_root.to_string_lossy().into_owned();
    pair.a
        .engine
        .share_collection(
            PROFILE.to_owned(),
            root.clone(),
            vec![pair.b.key(), c.key()],
        )
        .await
        .unwrap();
    eventually("entrées reçues", || {
        !pair.pending().is_empty() && !c.inbox.pending().unwrap().is_empty()
    })
    .await;

    let left = pair
        .a
        .engine
        .set_collection_device(PROFILE.to_owned(), root.clone(), pair.b.key(), false)
        .await
        .unwrap()
        .unwrap();

    assert!(left.peer(&pair.b.key()).is_none());
    assert!(left.peer(&c.key()).is_some());
    eventually("entrée de B retirée", || pair.pending().is_empty()).await;
    assert_eq!(c.inbox.pending().unwrap().len(), 1);
    // Retirer le dernier appareil supprime le partage
    let none = pair
        .a
        .engine
        .set_collection_device(PROFILE.to_owned(), root, c.key(), false)
        .await
        .unwrap();
    assert!(none.is_none());
    assert!(pair.a.store(PROFILE).shares().unwrap().is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_entry_withdrawn_while_the_device_was_absent_is_dropped_on_reconnection() {
    let pair = pair().await;
    pair.a.disk.put(&pair.a_root, "a.md", b"a");
    pair.start().await;
    pair.offer().await;
    eventually("entrée reçue", || !pair.pending().is_empty()).await;
    let root = pair.a_root.to_string_lossy().into_owned();
    pair.b.engine.set_paused(true).await.unwrap();

    pair.a
        .engine
        .set_collection_device(PROFILE.to_owned(), root, pair.b.key(), false)
        .await
        .unwrap();
    assert_eq!(pair.pending().len(), 1, "B n'a rien su");
    pair.b.engine.set_paused(false).await.unwrap();

    eventually("entrée retirée après la reconnexion", || {
        pair.pending().is_empty()
    })
    .await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn an_oversized_file_is_ignored_and_reported() {
    let pair = pair().await;
    pair.a.disk.put(&pair.a_root, "small.md", b"ok");
    let big = vec![b'x'; usize::try_from(MAX_FILE_BYTES).unwrap() + 1];
    pair.a.disk.put(&pair.a_root, "big.md", &big);
    pair.start().await;
    let share = pair.offer().await;
    pair.accept(share, false).await;

    eventually("petit fichier reçu", || pair.b_files() == ["small.md"]).await;

    eventually("fichier écarté signalé", || {
        let status = pair.a.engine.collection_status(&share, &[pair.b.key()]);
        status
            .ignored
            .iter()
            .any(|i| i.path == "big.md" && i.reason == IgnoredReason::TooLarge)
    })
    .await;
    assert_eq!(pair.b_files(), ["small.md"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn non_markdown_files_are_never_sent_nor_deleted() {
    let pair = pair().await;
    pair.a.disk.put(&pair.a_root, "a.md", b"a");
    pair.a.disk.put(&pair.a_root, "image.png", b"png");
    pair.b.disk.put(&pair.b_root, "keep.png", b"mine");
    pair.start().await;
    let share = pair.offer().await;
    pair.accept(share, true).await;

    eventually("copie", || text(&pair.b, &pair.b_root, "a.md").is_some()).await;

    assert!(text(&pair.b, &pair.b_root, "image.png").is_none());
    assert_eq!(
        text(&pair.b, &pair.b_root, "keep.png").as_deref(),
        Some("mine")
    );
    assert!(text(&pair.a, &pair.a_root, "keep.png").is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_removed_device_leaves_the_share_and_its_entries_disappear() {
    let pair = pair().await;
    pair.a.disk.put(&pair.a_root, "a.md", b"a");
    pair.start().await;
    let share = pair.offer().await;
    pair.accept(share, false).await;
    eventually("copie initiale", || pair.b_files() == ["a.md"]).await;
    let other = pair.offer_second().await;
    eventually("seconde entrée chez B", || !pair.pending().is_empty()).await;
    pair.a.events.taken();

    pair.a.engine.remove_device(pair.b.key()).await.unwrap();

    let store = pair.a.store(PROFILE);
    assert!(store.share(&share).unwrap().is_none());
    assert!(store.share(&other).unwrap().is_none());
    // Les fenêtres sont prévenues : l'état du partage et la liste des entrées sont à relire
    assert!(pair.a.has_event(|event| matches!(
        event,
        SyncEvent::CollectionChanged { share: changed, .. } if *changed == share
    )));
    assert!(
        pair.a
            .has_event(|event| matches!(event, SyncEvent::CollectionPendingChanged))
    );
}
