use app::ports::{FolderStore, NoteImporter, NoteStore};
use domain::folder::Folder;
use domain::note::Note;
use domain::transfer::plan_transfer;

use crate::SqliteStore;

fn folder(id: i64, parent_id: Option<i64>) -> Folder {
    Folder {
        id,
        name: format!("Dossier {id}"),
        parent_id,
        is_default: false,
        sort_order: Some(2),
        color: Some("#aa00ff".to_owned()),
        created_at: "2022-02-02 02:02:02".to_owned(),
    }
}

fn note(id: i64, folder_id: Option<i64>) -> Note {
    Note {
        id,
        title: format!("Note {id}"),
        content: format!("# Note {id}\nmot{id}"),
        tags: "[\"projet\"]".to_owned(),
        folder_id,
        sort_order: Some(3),
        color: Some("#00ff00".to_owned()),
        created_at: "2021-01-01 01:01:01".to_owned(),
        updated_at: "2021-03-03 03:03:03".to_owned(),
    }
}

fn target() -> SqliteStore {
    let store = SqliteStore::open_in_memory().unwrap();
    FolderStore::ensure_default(&store, "Inbox").unwrap();
    store
}

fn inbox(store: &SqliteStore) -> i64 {
    let folders = FolderStore::all(store).unwrap();
    folders.iter().find(|f| f.is_default).unwrap().id
}

#[test]
fn recreates_the_folder_tree_with_its_notes_dates_and_colors() {
    let store = target();
    let folders = [folder(40, None), folder(41, Some(40))];
    let notes = [note(7, Some(41))];
    let plan = plan_transfer(&folders, &notes, &[40], &[]);

    store.import(&plan.bundle).unwrap();

    let all = FolderStore::all(&store).unwrap();
    let top = all.iter().find(|f| f.name == "Dossier 40").unwrap();
    let child = all.iter().find(|f| f.name == "Dossier 41").unwrap();
    assert_eq!(top.parent_id, None);
    assert_eq!(top.sort_order, None);
    assert_eq!(child.parent_id, Some(top.id));
    assert_eq!(child.color.as_deref(), Some("#aa00ff"));
    assert_eq!(child.created_at, "2022-02-02 02:02:02");

    let copied = &NoteStore::all(&store).unwrap()[0];
    assert_eq!(copied.folder_id, Some(child.id));
    assert_eq!(copied.content, "# Note 7\nmot7");
    assert_eq!(copied.tags, "[\"projet\"]");
    assert_eq!(copied.color.as_deref(), Some("#00ff00"));
    assert_eq!(copied.created_at, "2021-01-01 01:01:01");
    assert_eq!(copied.updated_at, "2021-03-03 03:03:03");
}

#[test]
fn loose_notes_land_in_the_default_folder_and_are_searchable() {
    let store = target();
    let plan = plan_transfer(&[], &[note(3, Some(99))], &[], &[3]);

    store.import(&plan.bundle).unwrap();

    let found = NoteStore::search(&store, "mot3*").unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].folder_id, Some(inbox(&store)));
}

#[test]
fn creates_the_default_folder_of_a_never_opened_profile() {
    let store = SqliteStore::open_in_memory().unwrap();
    let plan = plan_transfer(&[], &[note(3, None)], &[], &[3]);

    store.import(&plan.bundle).unwrap();

    assert_eq!(
        NoteStore::all(&store).unwrap()[0].folder_id,
        Some(inbox(&store))
    );
}

#[test]
fn keeps_folders_with_a_name_already_taken() {
    let store = target();
    FolderStore::insert(&store, "Dossier 40", None).unwrap();
    let plan = plan_transfer(&[folder(40, None)], &[], &[40], &[]);

    store.import(&plan.bundle).unwrap();

    let same_name = FolderStore::all(&store)
        .unwrap()
        .into_iter()
        .filter(|f| f.name == "Dossier 40")
        .count();
    assert_eq!(same_name, 2);
}

/// Surveillance et disque inertes : le transfert d'une collection ne touche que la liste.
struct Inert;

impl app::ports::CollectionWatcher for Inert {
    fn watch(&self, _root: &str, _kind: domain::collection::CollectionKind) {}
    fn unwatch(&self, _root: &str) {}
    fn set_excludes(&self, _excludes: Vec<String>) {}
    fn search(&self, _request_id: u64, _query: String) {}
    fn cancel_search(&self, _request_id: u64) {}
    fn graph(&self, _root: &str) {}
}

impl app::ports::CollectionFiles for Inert {
    fn entry_kind(&self, _path: &std::path::Path) -> Option<app::ports::EntryKind> {
        None
    }
    fn read_text(&self, _path: &std::path::Path) -> app::AppResult<String> {
        Ok(String::new())
    }
    fn read_bytes(&self, _path: &std::path::Path) -> app::AppResult<Vec<u8>> {
        Ok(Vec::new())
    }
    fn write_text(&self, _path: &std::path::Path, _content: &str) -> app::AppResult<()> {
        Ok(())
    }
    fn create_new(&self, _path: &std::path::Path) -> app::AppResult<()> {
        Ok(())
    }
    fn rename(&self, _from: &std::path::Path, _to: &std::path::Path) -> app::AppResult<()> {
        Ok(())
    }
    fn trash(&self, _path: &std::path::Path) -> app::AppResult<()> {
        Ok(())
    }
}

/// D'un profil à l'autre, avec deux vraies bases : un dossier (et son sous-dossier), une note
/// seule et une collection partent ; le reste ne bouge pas.
#[test]
fn moves_items_between_two_real_databases() {
    use std::sync::Arc;

    use app::collections::{CollectionPorts, CollectionsService};
    use app::ports::CollectionStore;
    use app::profiles::transfer::{TransferRequest, TransferSource, TransferTarget, transfer};
    use domain::collection::CollectionKind;

    let source = Arc::new(target());
    let destination = target();
    let project = FolderStore::insert(source.as_ref(), "Projet", None).unwrap();
    let sub = FolderStore::insert(source.as_ref(), "Sous", Some(project.id)).unwrap();
    let new_note = |title: &str, folder_id| domain::note::NewNote {
        title: title.to_owned(),
        content: format!("# {title}"),
        tags: vec![],
        folder_id,
    };
    let inside = NoteStore::insert(source.as_ref(), &new_note("Dedans", Some(sub.id))).unwrap();
    let loose = NoteStore::insert(source.as_ref(), &new_note("Seule", None)).unwrap();
    let kept = NoteStore::insert(source.as_ref(), &new_note("Reste", None)).unwrap();
    CollectionStore::add(source.as_ref(), "C:/docs", CollectionKind::Folder, "Docs").unwrap();
    let collections = CollectionsService::new(CollectionPorts {
        store: source.clone(),
        files: Arc::new(Inert),
        watcher: Arc::new(Inert),
    });
    let request = TransferRequest {
        note_ids: vec![loose.id],
        folder_ids: vec![project.id],
        collections: vec!["C:/docs".to_owned()],
    };

    let moved = transfer(
        &TransferSource {
            notes: source.as_ref(),
            folders: source.as_ref(),
            collections: &collections,
        },
        &TransferTarget {
            importer: &destination,
            collections: &destination,
        },
        &request,
    )
    .unwrap();

    assert_eq!(moved.note_ids, [inside.id, loose.id]);
    assert_eq!(moved.folder_ids, [project.id, sub.id]);
    let left: Vec<i64> = NoteStore::all(source.as_ref())
        .unwrap()
        .iter()
        .map(|n| n.id)
        .collect();
    assert_eq!(left, [kept.id]);
    assert!(
        FolderStore::by_id(source.as_ref(), project.id)
            .unwrap()
            .is_none()
    );
    assert!(CollectionStore::all(source.as_ref()).unwrap().is_empty());

    let titles: Vec<String> = NoteStore::all(&destination)
        .unwrap()
        .into_iter()
        .map(|n| n.title)
        .collect();
    assert_eq!(titles.len(), 2);
    assert!(titles.contains(&"Dedans".to_owned()) && titles.contains(&"Seule".to_owned()));
    assert_eq!(FolderStore::all(&destination).unwrap().len(), 3);
    assert_eq!(
        CollectionStore::all(&destination).unwrap()[0].display_name,
        "Docs"
    );
}
