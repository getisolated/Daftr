use std::path::{Path, PathBuf};

use app::collection_sync::{
    FileAction, FileEntry, FileHash, IgnoredReason, PlannedFile, Role, SyncPath,
};
use app::ports::collection_sync::{BaseRow, CollectionSyncStore};
use app::sync::testing::{MemoryDisk, MemorySyncStore};
use domain::sync::DeviceKey;
use domain::uuid::Uuid;

use super::*;

const ROOT: &str = "/docs";

struct Fixture {
    disk: MemoryDisk,
    store: MemorySyncStore,
    root: PathBuf,
    id: Uuid,
    peer: DeviceKey,
}

impl Fixture {
    fn new() -> Self {
        let peer = DeviceKey::from_bytes([2; 32]);
        let store = MemorySyncStore::new(DeviceKey::from_bytes([1; 32]));
        store.list_collection(ROOT, "Docs");
        let id = Uuid::from_random([9; 16]);
        store.create_share(ROOT, &id, &[peer]).unwrap();
        let disk = MemoryDisk::default();
        disk.create_directory(Path::new(ROOT));
        Self {
            disk,
            store,
            root: PathBuf::from(ROOT),
            id,
            peer,
        }
    }

    fn ports(&self) -> Ports<'_> {
        Ports {
            disk: &self.disk,
            store: &self.store,
        }
    }

    fn ctx(&self, role: Role) -> ShareCtx<'_> {
        ShareCtx {
            id: self.id,
            root: &self.root,
            remote: self.peer,
            role,
            my_name: "Bureau",
        }
    }

    fn put(&self, path: &str, text: &str) {
        self.disk.put(&self.root, path, text.as_bytes());
    }

    fn text(&self, path: &str) -> Option<String> {
        self.disk.text(&self.root, path)
    }

    fn base(&self, path: &str) -> Option<BaseRow> {
        self.store.base(&self.id, &self.peer, path).unwrap()
    }
}

fn path(text: &str) -> SyncPath {
    SyncPath::parse(text, &[]).unwrap()
}

fn hash(text: &str) -> FileHash {
    FileHash::of(text.as_bytes())
}

fn planned(
    action: FileAction,
    name: &str,
    local: Option<&str>,
    remote: Option<&str>,
) -> PlannedFile {
    PlannedFile {
        path: path(name),
        action,
        local: local.map(hash),
        remote: remote.map(hash),
        base: None,
        size: remote.map_or(0, |text| text.len() as u64),
    }
}

#[test]
fn the_snapshot_hashes_files_and_lists_deletions() {
    let fixture = Fixture::new();
    fixture.put("a.md", "alpha");
    fixture.put("docs/b.md", "beta");
    fixture
        .store
        .set_base(
            &fixture.id,
            &fixture.peer,
            "gone.md",
            &BaseRow {
                hash: hash("old"),
                text: None,
            },
        )
        .unwrap();

    let snapshot = snapshot(
        &fixture.ports(),
        &fixture.ctx(Role::Arbiter),
        1_000_000_000_000,
    )
    .unwrap()
    .unwrap();

    let entries: Vec<(&str, Option<FileHash>)> = snapshot
        .manifest
        .entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry.hash))
        .collect();
    assert_eq!(
        entries,
        [
            ("a.md", Some(hash("alpha"))),
            ("docs/b.md", Some(hash("beta"))),
            ("gone.md", None)
        ]
    );
    assert_eq!(snapshot.summary.files, 2);
    let index = fixture.store.file_index(&fixture.id).unwrap();
    assert_eq!(index["a.md"].hash, hash("alpha"));
}

#[test]
fn a_missing_folder_has_no_snapshot() {
    let fixture = Fixture::new();
    let ctx = ShareCtx {
        root: Path::new("/elsewhere"),
        ..fixture.ctx(Role::Arbiter)
    };

    assert!(snapshot(&fixture.ports(), &ctx, 0).unwrap().is_none());
}

#[test]
fn the_summary_lists_conflict_copies_and_ignored_files() {
    let fixture = Fixture::new();
    fixture.put("plan.md", "a");
    fixture.put("plan (conflit Portable).md", "b");
    fixture.put("orphan (conflit Portable).md", "c");
    fixture.put("Case.md", "x");
    fixture.put("case.md", "y");

    let summary = snapshot(
        &fixture.ports(),
        &fixture.ctx(Role::Arbiter),
        1_000_000_000_000,
    )
    .unwrap()
    .unwrap()
    .summary;

    assert_eq!(summary.conflicts, ["plan (conflit Portable).md"]);
    assert_eq!(summary.ignored.len(), 2);
    assert!(
        summary
            .ignored
            .iter()
            .all(|i| i.reason == IgnoredReason::CaseCollision)
    );
}

#[test]
fn evaluating_records_identical_files_and_queues_the_rest() {
    let fixture = Fixture::new();
    fixture.put("same.md", "x");
    fixture.put("mine.md", "m");
    let remote = vec![
        FileEntry {
            path: path("same.md"),
            hash: Some(hash("x")),
            size: 1,
            base: None,
        },
        FileEntry {
            path: path("theirs.md"),
            hash: Some(hash("t")),
            size: 1,
            base: None,
        },
    ];

    let evaluation = evaluate(&fixture.ports(), &fixture.ctx(Role::Arbiter), &remote)
        .unwrap()
        .unwrap();

    assert_eq!(evaluation.queue.len(), 1);
    assert_eq!(evaluation.queue[0].path, path("theirs.md"));
    assert_eq!(fixture.base("same.md").unwrap().hash, hash("x"));
    assert!(fixture.base("mine.md").is_none());
}

#[test]
fn a_pulled_file_is_written_and_remembered() {
    let fixture = Fixture::new();
    let item = planned(FileAction::Pull, "docs/new.md", None, Some("hello"));

    let outcome = pull(
        &fixture.ports(),
        &fixture.ctx(Role::Arbiter),
        &item,
        b"hello",
    )
    .unwrap();

    assert_eq!(fixture.text("docs/new.md").as_deref(), Some("hello"));
    assert_eq!(outcome.ack, Some(Some(hash("hello"))));
    assert!(outcome.changed && !outcome.skipped);
    let base = fixture.base("docs/new.md").unwrap();
    assert_eq!(
        (base.hash, base.text),
        (hash("hello"), Some(b"hello".to_vec()))
    );
    assert_eq!(
        fixture.store.file_index(&fixture.id).unwrap()["docs/new.md"].hash,
        hash("hello")
    );
}

#[test]
fn a_pull_never_overwrites_a_file_edited_since_the_plan() {
    let fixture = Fixture::new();
    fixture.put("a.md", "edited by me");
    let item = planned(FileAction::Pull, "a.md", Some("old"), Some("new"));

    let outcome = pull(&fixture.ports(), &fixture.ctx(Role::Arbiter), &item, b"new").unwrap();

    assert!(outcome.skipped);
    assert_eq!(fixture.text("a.md").as_deref(), Some("edited by me"));
    assert!(fixture.base("a.md").is_none());
}

#[test]
fn a_pull_refuses_content_that_is_not_the_announced_version() {
    let fixture = Fixture::new();
    let item = planned(FileAction::Pull, "a.md", None, Some("announced"));

    let outcome = pull(
        &fixture.ports(),
        &fixture.ctx(Role::Arbiter),
        &item,
        b"tampered",
    )
    .unwrap();

    assert!(outcome.skipped);
    assert!(fixture.text("a.md").is_none());
}

#[test]
fn a_remote_deletion_sends_the_file_to_the_trash() {
    let fixture = Fixture::new();
    fixture.put("gone.md", "bye");
    fixture
        .store
        .set_base(
            &fixture.id,
            &fixture.peer,
            "gone.md",
            &BaseRow {
                hash: hash("bye"),
                text: None,
            },
        )
        .unwrap();
    let item = planned(FileAction::DeleteLocal, "gone.md", Some("bye"), None);

    let outcome = delete_local(&fixture.ports(), &fixture.ctx(Role::Arbiter), &item).unwrap();

    assert!(fixture.text("gone.md").is_none());
    assert_eq!(fixture.disk.trashed(&fixture.root), ["gone.md"]);
    assert_eq!(outcome.ack, Some(None));
    assert!(fixture.base("gone.md").is_none());
    assert!(
        !fixture
            .store
            .file_index(&fixture.id)
            .unwrap()
            .contains_key("gone.md")
    );
}

#[test]
fn a_deletion_spares_a_file_edited_since_the_plan() {
    let fixture = Fixture::new();
    fixture.put("a.md", "edited");
    let item = planned(FileAction::DeleteLocal, "a.md", Some("old"), None);

    let outcome = delete_local(&fixture.ports(), &fixture.ctx(Role::Arbiter), &item).unwrap();

    assert!(outcome.skipped);
    assert_eq!(fixture.text("a.md").as_deref(), Some("edited"));
    assert!(fixture.disk.trashed(&fixture.root).is_empty());
}

fn conflict(fixture: &Fixture, role: Role, local: &str, remote: &str) -> Outcome {
    fixture.put("n.md", local);
    let mut item = planned(FileAction::Reconcile, "n.md", Some(local), Some(remote));
    item.base = Some(hash("un\ndeux\n"));
    fixture
        .store
        .set_base(
            &fixture.id,
            &fixture.peer,
            "n.md",
            &BaseRow {
                hash: hash("un\ndeux\n"),
                text: Some(b"un\ndeux\n".to_vec()),
            },
        )
        .unwrap();
    reconcile(
        &fixture.ports(),
        &fixture.ctx(role),
        &item,
        remote.as_bytes(),
    )
    .unwrap()
}

#[test]
fn disjoint_edits_are_merged_the_same_way_on_both_roles() {
    let (mine, theirs) = ("UN\ndeux\n", "un\ndeux\ntrois\n");
    let arbiter = Fixture::new();
    let follower = Fixture::new();

    conflict(&arbiter, Role::Arbiter, mine, theirs);
    conflict(&follower, Role::Follower, theirs, mine);

    assert_eq!(arbiter.text("n.md").as_deref(), Some("UN\ndeux\ntrois\n"));
    assert_eq!(arbiter.text("n.md"), follower.text("n.md"));
    assert_eq!(arbiter.base("n.md"), follower.base("n.md"));
    assert_eq!(arbiter.disk.paths(&arbiter.root), ["n.md"]);
}

#[test]
fn without_a_common_text_the_follower_keeps_its_version_in_a_copy() {
    let fixture = Fixture::new();
    fixture.put("x.md", "from the follower");
    let item = planned(
        FileAction::Reconcile,
        "x.md",
        Some("from the follower"),
        Some("from the arbiter"),
    );

    let outcome = reconcile(
        &fixture.ports(),
        &fixture.ctx(Role::Follower),
        &item,
        b"from the arbiter",
    )
    .unwrap();

    assert_eq!(fixture.text("x.md").as_deref(), Some("from the arbiter"));
    assert_eq!(
        fixture.text("x (conflit Bureau).md").as_deref(),
        Some("from the follower")
    );
    assert_eq!(
        outcome.conflict_copy.as_deref(),
        Some("x (conflit Bureau).md")
    );
    assert_eq!(outcome.ack, Some(Some(hash("from the arbiter"))));
    assert_eq!(outcome.replaced.len(), 1);
}

#[test]
fn without_a_common_text_the_arbiter_changes_nothing() {
    let fixture = Fixture::new();
    fixture.put("x.md", "from the arbiter");
    let item = planned(
        FileAction::Reconcile,
        "x.md",
        Some("from the arbiter"),
        Some("from the follower"),
    );

    let outcome = reconcile(
        &fixture.ports(),
        &fixture.ctx(Role::Arbiter),
        &item,
        b"from the follower",
    )
    .unwrap();

    assert_eq!(outcome, Outcome::default());
    assert_eq!(fixture.disk.paths(&fixture.root), ["x.md"]);
}

#[test]
fn a_taken_copy_name_gets_a_number() {
    let fixture = Fixture::new();
    fixture.put("x.md", "mine");
    fixture.put("x (conflit Bureau).md", "an older copy");
    let item = planned(FileAction::Reconcile, "x.md", Some("mine"), Some("theirs"));

    reconcile(
        &fixture.ports(),
        &fixture.ctx(Role::Follower),
        &item,
        b"theirs",
    )
    .unwrap();

    assert_eq!(
        fixture.text("x (conflit Bureau).md").as_deref(),
        Some("an older copy")
    );
    assert_eq!(
        fixture.text("x (conflit Bureau 2).md").as_deref(),
        Some("mine")
    );
}

#[test]
fn an_acknowledgement_keeps_the_text_only_while_the_file_is_unchanged() {
    let fixture = Fixture::new();
    fixture.put("a.md", "v1");

    acked(
        &fixture.ports(),
        &fixture.ctx(Role::Arbiter),
        &path("a.md"),
        Some(hash("v1")),
    )
    .unwrap();
    assert_eq!(fixture.base("a.md").unwrap().text, Some(b"v1".to_vec()));

    fixture.put("a.md", "v2");
    acked(
        &fixture.ports(),
        &fixture.ctx(Role::Arbiter),
        &path("a.md"),
        Some(hash("v1")),
    )
    .unwrap();
    let base = fixture.base("a.md").unwrap();
    assert_eq!((base.hash, base.text), (hash("v1"), None));

    fixture.disk.remove(&fixture.root, "a.md");
    acked(
        &fixture.ports(),
        &fixture.ctx(Role::Arbiter),
        &path("a.md"),
        None,
    )
    .unwrap();
    assert!(fixture.base("a.md").is_none());
}

#[test]
fn a_deletion_acknowledgement_spares_a_file_recreated_since() {
    let fixture = Fixture::new();
    fixture.put("a.md", "back");
    fixture
        .store
        .set_base(
            &fixture.id,
            &fixture.peer,
            "a.md",
            &BaseRow {
                hash: hash("old"),
                text: None,
            },
        )
        .unwrap();

    acked(
        &fixture.ports(),
        &fixture.ctx(Role::Arbiter),
        &path("a.md"),
        None,
    )
    .unwrap();

    assert!(fixture.base("a.md").is_some());
}

#[test]
fn the_history_serves_a_replaced_version_then_forgets_the_oldest() {
    let mut history = History::default();
    history.remember("a.md", hash("v1"), b"v1".to_vec());

    assert_eq!(history.find("a.md", &hash("v1")).unwrap().as_slice(), b"v1");
    assert!(history.find("a.md", &hash("v2")).is_none());
    assert!(history.find("b.md", &hash("v1")).is_none());

    for number in 0..100 {
        history.remember("c.md", hash(&number.to_string()), vec![0]);
    }
    assert!(history.find("a.md", &hash("v1")).is_none());
    assert!(history.find("c.md", &hash("99")).is_some());
}
