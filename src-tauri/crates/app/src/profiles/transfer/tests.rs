use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use domain::collection::CollectionKind;
use domain::transfer::TransferBundle;
use platform::sync::lock;

use super::*;
use crate::AppError;
use crate::collections::CollectionPorts;
use crate::testing::{
    FakeFiles, MemoryCollections, MemoryFolders, MemoryNotes, RecordingWatcher, folder, note,
};

/// Base cible qui garde ce qu'elle reçoit, ou refuse tout.
#[derive(Default)]
struct RecordingImporter {
    imported: Mutex<Vec<TransferBundle>>,
    failing: bool,
}

impl NoteImporter for RecordingImporter {
    fn import(&self, bundle: &TransferBundle) -> AppResult<()> {
        if self.failing {
            return Err(AppError::storage("disk full"));
        }
        lock(&self.imported).push(bundle.clone());
        Ok(())
    }
}

struct Fixture {
    notes: MemoryNotes,
    folders: MemoryFolders,
    collections: CollectionsService,
    source_collections: Arc<MemoryCollections>,
    watcher: Arc<RecordingWatcher>,
}

/// Inbox (1) ; Projets (2) > Client (3) ; notes 10 (Inbox), 11 (Projets), 12 (Client), 13.
fn fixture() -> Fixture {
    let source_collections = Arc::new(MemoryCollections::default());
    let watcher = Arc::new(RecordingWatcher::default());
    let collections = CollectionsService::new(CollectionPorts {
        store: source_collections.clone(),
        files: Arc::new(FakeFiles::default()),
        watcher: watcher.clone(),
    });
    Fixture {
        notes: MemoryNotes::with(vec![
            note(10, Some(1), "[]"),
            note(11, Some(2), "[]"),
            note(12, Some(3), "[]"),
            note(13, None, "[]"),
        ]),
        folders: MemoryFolders::with(vec![
            folder(1, None, true),
            folder(2, None, false),
            folder(3, Some(2), false),
        ]),
        collections,
        source_collections,
        watcher,
    }
}

impl Fixture {
    fn source(&self) -> TransferSource<'_> {
        TransferSource {
            notes: &self.notes,
            folders: &self.folders,
            collections: &self.collections,
        }
    }

    fn note_calls(&self) -> Vec<String> {
        lock(&self.notes.calls)
            .iter()
            .filter(|call| call.starts_with("delete"))
            .cloned()
            .collect()
    }

    fn folder_calls(&self) -> Vec<String> {
        lock(&self.folders.calls)
            .iter()
            .filter(|call| call.starts_with("delete"))
            .cloned()
            .collect()
    }
}

fn request(note_ids: &[i64], folder_ids: &[i64], collections: &[&str]) -> TransferRequest {
    TransferRequest {
        note_ids: note_ids.to_vec(),
        folder_ids: folder_ids.to_vec(),
        collections: collections.iter().map(|path| (*path).to_owned()).collect(),
    }
}

#[test]
fn copies_then_deletes_a_folder_and_a_loose_note() {
    let fixture = fixture();
    let importer = RecordingImporter::default();
    let target_collections = MemoryCollections::default();
    let target = TransferTarget {
        importer: &importer,
        collections: &target_collections,
    };

    let moved = transfer(&fixture.source(), &target, &request(&[13], &[2], &[])).unwrap();

    let bundles = lock(&importer.imported);
    assert_eq!(bundles.len(), 1, "une seule transaction dans la cible");
    assert_eq!(bundles[0].folders.len(), 2);
    assert_eq!(bundles[0].notes.len(), 3);
    assert_eq!(
        fixture.note_calls(),
        ["delete 11", "delete 12", "delete 13"]
    );
    assert_eq!(fixture.folder_calls(), ["delete 2 moveToRoot"]);
    assert_eq!(moved.note_ids, [11, 12, 13]);
    assert_eq!(moved.folder_ids, [2, 3]);
}

#[test]
fn deletes_nothing_when_the_copy_fails() {
    let fixture = fixture();
    let importer = RecordingImporter {
        failing: true,
        ..RecordingImporter::default()
    };
    let target_collections = MemoryCollections::default();
    let target = TransferTarget {
        importer: &importer,
        collections: &target_collections,
    };

    let result = transfer(&fixture.source(), &target, &request(&[13], &[2], &[]));

    assert!(result.is_err());
    assert!(fixture.folder_calls().is_empty());
    assert!(fixture.note_calls().is_empty());
}

#[test]
fn does_nothing_for_the_default_folder_or_unknown_items() {
    let fixture = fixture();
    let importer = RecordingImporter::default();
    let target_collections = MemoryCollections::default();
    let target = TransferTarget {
        importer: &importer,
        collections: &target_collections,
    };

    let moved = transfer(&fixture.source(), &target, &request(&[99], &[1], &["nope"])).unwrap();

    assert_eq!(moved, Transferred::default());
    assert!(lock(&importer.imported).is_empty());
}

#[test]
fn moves_the_record_of_a_collection_and_stops_watching_it() {
    let fixture = fixture();
    let path = PathBuf::from("C:/notes").to_string_lossy().into_owned();
    fixture
        .source_collections
        .add(&path, CollectionKind::Folder, "Notes")
        .unwrap();
    fixture.collections.load(&path).unwrap();
    let importer = RecordingImporter::default();
    let target_collections = MemoryCollections::default();
    let target = TransferTarget {
        importer: &importer,
        collections: &target_collections,
    };

    let moved = transfer(&fixture.source(), &target, &request(&[], &[], &[&path])).unwrap();

    assert_eq!(moved.collections, std::slice::from_ref(&path));
    let received = target_collections.get(&path).unwrap().unwrap();
    assert_eq!(received.display_name, "Notes");
    assert_eq!(fixture.source_collections.get(&path).unwrap(), None);
    assert!(fixture.watcher.calls().contains(&format!("unwatch {path}")));
}

#[test]
fn a_collection_already_in_the_target_is_not_added_twice() {
    let fixture = fixture();
    fixture
        .source_collections
        .add("C:/Notes", CollectionKind::Folder, "Mine")
        .unwrap();
    let importer = RecordingImporter::default();
    let target_collections = MemoryCollections::default();
    target_collections
        .add("c:/notes", CollectionKind::Folder, "Theirs")
        .unwrap();
    let target = TransferTarget {
        importer: &importer,
        collections: &target_collections,
    };

    transfer(
        &fixture.source(),
        &target,
        &request(&[], &[], &["C:/Notes"]),
    )
    .unwrap();

    assert_eq!(target_collections.all().unwrap().len(), 1);
    assert_eq!(fixture.source_collections.all().unwrap().len(), 0);
}
