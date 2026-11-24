//! Collections partagées entre deux moteurs réels sur loopback : proposition, acceptation,
//! échange des fichiers, suppressions, conflits, fusion avec un dossier existant, refus.

use std::path::{Path, PathBuf};

use app::collection_sync::{IgnoredReason, MAX_FILE_BYTES};
use app::ports::collection_sync::{CollectionDisk, CollectionSyncStore};
use app::ports::sync::SyncEvent;
use domain::uuid::Uuid;

use super::support::{Device, device, eventually, link, share, space};
use crate::{CollectionAnswer, MemoryNetwork};

const PROFILE: &str = "p";

/// A et B partagent le profil `p` ; A a la collection `docs`.
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
    a.create_space(PROFILE, space(1));
    share(&a, PROFILE, &b, PROFILE);
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

    async fn accept(&self, share: Uuid, merge: bool) {
        eventually("proposition reçue", || {
            !self.b.engine.pending_collections().is_empty()
        })
        .await;
        let answer = CollectionAnswer::Accept {
            destination: self.b_root.clone(),
            merge,
        };
        self.b
            .engine
            .answer_collection(self.a.key(), share, answer)
            .await
            .unwrap();
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
                if offers[0].1.files == 2 && offers[0].1.name == "Docs")
        })
    })
    .await;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    assert!(pair.b_files().is_empty(), "rien avant l'acceptation");
    assert!(pair.b.store(PROFILE).shares().unwrap().is_empty());
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
    let record = pair.b.store(PROFILE).share(&share).unwrap().unwrap();
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
        .collection_changed(PROFILE.to_owned(), share)
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
    eventually("proposition reçue", || {
        !pair.b.engine.pending_collections().is_empty()
    })
    .await;

    let refused = pair
        .b
        .engine
        .answer_collection(
            pair.a.key(),
            share,
            CollectionAnswer::Accept {
                destination: pair.b_root.clone(),
                merge: false,
            },
        )
        .await;

    assert!(refused.is_err());
    assert_eq!(pair.b_files(), ["existing.md"]);
    assert!(pair.b.store(PROFILE).shares().unwrap().is_empty());
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
        let store = pair.b.store(PROFILE);
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
                .store(PROFILE)
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
    eventually("proposition reçue", || {
        !pair.b.engine.pending_collections().is_empty()
    })
    .await;

    pair.b
        .engine
        .answer_collection(pair.a.key(), share, CollectionAnswer::Decline)
        .await
        .unwrap();

    eventually("partage retiré chez A", || {
        pair.a.store(PROFILE).share(&share).unwrap().is_none()
    })
    .await;
    assert!(pair.b.engine.pending_collections().is_empty());
    assert!(pair.b_files().is_empty());
    // Une nouvelle proposition reste possible
    let again = pair.offer().await;
    eventually("nouvelle proposition", || {
        !pair.b.engine.pending_collections().is_empty()
    })
    .await;
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
        pair.b.store(PROFILE).share(&share).unwrap().is_none()
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
async fn only_devices_sharing_the_profile_can_receive_a_collection() {
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
        .share_collection(PROFILE.to_owned(), root, vec![])
        .await;

    assert!(refused.is_err() && nobody.is_err());
    assert!(pair.a.store(PROFILE).shares().unwrap().is_empty());
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
async fn a_removed_device_leaves_the_share() {
    let pair = pair().await;
    pair.a.disk.put(&pair.a_root, "a.md", b"a");
    pair.start().await;
    let share = pair.offer().await;
    pair.accept(share, false).await;
    eventually("copie initiale", || pair.b_files() == ["a.md"]).await;

    pair.a
        .engine
        .stop_sharing(PROFILE.to_owned(), pair.b.key())
        .await
        .unwrap();

    let store = pair.a.store(PROFILE);
    let record = store.share(&share).unwrap();
    assert!(record.is_none_or(|record| record.peer(&pair.b.key()).is_none()));
}
