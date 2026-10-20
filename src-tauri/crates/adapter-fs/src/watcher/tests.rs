use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use app::collections::CollectionEvent;
use app::ports::{CollectionEventSink, CollectionWatcher};
use domain::collection::CollectionKind;

use super::CollectionsWatcher;

const WAIT: Duration = Duration::from_secs(5);

/// Transmet les événements au test.
struct ChannelSink(Mutex<Sender<CollectionEvent>>);

impl CollectionEventSink for ChannelSink {
    fn publish(&self, event: CollectionEvent) {
        self.0.lock().unwrap().send(event).unwrap();
    }
}

fn start() -> (CollectionsWatcher, Receiver<CollectionEvent>) {
    let (sender, receiver) = mpsc::channel();
    let watcher =
        CollectionsWatcher::start(Arc::new(ChannelSink(Mutex::new(sender))), Vec::new()).unwrap();
    (watcher, receiver)
}

fn next(receiver: &Receiver<CollectionEvent>) -> CollectionEvent {
    receiver.recv_timeout(WAIT).expect("événement attendu")
}

/// Attend le premier événement qui vérifie `predicate`, en ignorant les autres.
fn wait_for(
    receiver: &Receiver<CollectionEvent>,
    predicate: impl Fn(&CollectionEvent) -> bool,
) -> CollectionEvent {
    loop {
        let event = next(receiver);
        if predicate(&event) {
            return event;
        }
    }
}

fn folder_with(files: &[(&str, &str)]) -> (tempfile::TempDir, String) {
    let directory = tempfile::tempdir().unwrap();
    for (name, content) in files {
        let path = directory.path().join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }
    let root = platform::paths::canonical_path(directory.path())
        .to_string_lossy()
        .into_owned();
    (directory, root)
}

#[test]
fn sends_the_tree_of_a_watched_folder() {
    let (_directory, root) = folder_with(&[("a.md", ""), ("docs/b.md", ""), ("c.txt", "")]);
    let (watcher, events) = start();

    watcher.watch(&root, CollectionKind::Folder);

    let CollectionEvent::Tree {
        tree, truncated, ..
    } = next(&events)
    else {
        panic!("arbre attendu");
    };
    let names: Vec<&str> = tree.iter().map(|node| node.name.as_str()).collect();
    assert_eq!(names, ["docs", "a.md"]);
    assert!(!truncated);
}

#[test]
fn reports_a_missing_folder() {
    let (watcher, events) = start();
    let missing = std::env::temp_dir().join("evn-missing-collection-folder");

    watcher.watch(&missing.to_string_lossy(), CollectionKind::Folder);

    assert!(matches!(next(&events), CollectionEvent::Error { .. }));
}

#[test]
fn reports_changed_markdown_files() {
    let (directory, root) = folder_with(&[("a.md", "old")]);
    let (watcher, events) = start();
    watcher.watch(&root, CollectionKind::Folder);
    next(&events);

    std::fs::write(directory.path().join("a.md"), "new").unwrap();

    let changed = wait_for(&events, |event| {
        matches!(event, CollectionEvent::Changed { .. })
    });
    assert_eq!(
        changed,
        CollectionEvent::Changed {
            root,
            paths: vec!["a.md".into()]
        }
    );
}

#[test]
fn searches_names_and_contents_then_finishes() {
    let (_directory, root) = folder_with(&[("guide.md", "nothing"), ("other.md", "see the GUIDE")]);
    let (watcher, events) = start();
    watcher.watch(&root, CollectionKind::Folder);
    next(&events);

    watcher.search(7, "guide".into());

    let mut found = Vec::new();
    loop {
        match next(&events) {
            CollectionEvent::SearchResults {
                done: true,
                request_id,
                ..
            } => {
                assert_eq!(request_id, 7);
                break;
            }
            CollectionEvent::SearchResults { path, matches, .. } => {
                found.push((path, matches.len()))
            }
            _ => {}
        }
    }
    found.sort();
    assert_eq!(
        found,
        [("guide.md".to_owned(), 0), ("other.md".to_owned(), 1)]
    );
}

#[test]
fn builds_the_graph_of_a_collection() {
    let (_directory, root) = folder_with(&[("a.md", "[b](b.md)"), ("b.md", "")]);
    let (watcher, events) = start();
    watcher.watch(&root, CollectionKind::Folder);
    next(&events);

    watcher.graph(&root);

    let CollectionEvent::Graph { graph, .. } =
        wait_for(&events, |e| matches!(e, CollectionEvent::Graph { .. }))
    else {
        unreachable!();
    };
    assert_eq!(graph.links.len(), 1);
}

#[test]
fn a_new_exclude_list_rescans_the_collections() {
    let (_directory, root) = folder_with(&[("a.md", ""), ("drafts/b.md", "")]);
    let (watcher, events) = start();
    watcher.watch(&root, CollectionKind::Folder);
    next(&events);

    watcher.set_excludes(vec!["drafts".into()]);

    let CollectionEvent::Tree { tree, .. } =
        wait_for(&events, |e| matches!(e, CollectionEvent::Tree { .. }))
    else {
        unreachable!();
    };
    assert_eq!(tree.len(), 1);
}
