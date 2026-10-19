use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::collection::{CollectionFileRef, CollectionKind};
use platform::sync::lock;

use super::{CollectionPorts, CollectionsService};
use crate::testing::{FakeFiles, MemoryCollections, RecordingWatcher};
use crate::{AppError, Refusal};

fn base() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(r"C:\fake")
    } else {
        PathBuf::from("/fake")
    }
}

struct Fixture {
    files: Arc<FakeFiles>,
    watcher: Arc<RecordingWatcher>,
    service: CollectionsService,
    repo: PathBuf,
}

/// Un dossier `repo` (avec `docs/guide.md` et `README.md`) et un fichier `notes.md`.
fn fixture() -> Fixture {
    let files = Arc::new(FakeFiles::default());
    let watcher = Arc::new(RecordingWatcher::default());
    let repo = base().join("repo");
    files.add_dir(&repo);
    files.add_dir(&repo.join("docs"));
    files.add_file(&repo.join("docs").join("guide.md"), "# Guide");
    files.add_file(&repo.join("README.md"), "# Readme");
    files.add_file(&base().join("notes.md"), "notes");
    files.add_file(&base().join("image.png"), "");

    let ports = CollectionPorts {
        store: Arc::new(MemoryCollections::default()),
        files: files.clone(),
        watcher: watcher.clone(),
    };
    Fixture {
        files,
        watcher,
        service: CollectionsService::new(ports),
        repo,
    }
}

fn root_of(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn refusal(result: Result<impl std::fmt::Debug, AppError>) -> Refusal {
    match result {
        Err(AppError::Refused(refusal)) => refusal,
        other => panic!("refus attendu, obtenu {other:?}"),
    }
}

#[test]
fn opens_folders_and_markdown_files_only() {
    let fixture = fixture();

    let folder = fixture.service.add(&fixture.repo).unwrap().unwrap();
    let file = fixture
        .service
        .add(&base().join("notes.md"))
        .unwrap()
        .unwrap();

    assert_eq!(
        (folder.kind, folder.display_name.as_str()),
        (CollectionKind::Folder, "repo")
    );
    assert_eq!(file.kind, CollectionKind::File);
    assert!(
        fixture
            .service
            .add(&base().join("image.png"))
            .unwrap()
            .is_none()
    );
    assert!(
        fixture
            .service
            .add(&base().join("missing"))
            .unwrap()
            .is_none()
    );
}

#[test]
fn the_same_folder_opened_twice_is_one_collection() {
    let fixture = fixture();
    let first = fixture.service.add(&fixture.repo).unwrap().unwrap();

    let upper = PathBuf::from(root_of(&fixture.repo).to_uppercase());
    let second = fixture.service.add(&upper).unwrap().unwrap();

    assert_eq!(first.path, second.path);
    assert_eq!(fixture.service.list().unwrap().len(), 1);
}

#[test]
fn refuses_an_unknown_collection() {
    let fixture = fixture();
    assert_eq!(
        refusal(fixture.service.read("nowhere", "a.md")),
        Refusal::UnknownCollection
    );
}

#[test]
fn refuses_paths_outside_of_the_collection() {
    let fixture = fixture();
    let root = root_of(&fixture.repo);
    fixture.service.add(&fixture.repo).unwrap();

    assert_eq!(
        refusal(fixture.service.read(&root, "../notes.md")),
        Refusal::OutsideCollection
    );
    assert_eq!(
        fixture.service.read(&root, "docs/guide.md").unwrap(),
        "# Guide"
    );
}

#[test]
fn a_file_collection_only_holds_its_own_file() {
    let fixture = fixture();
    let file = fixture
        .service
        .add(&base().join("notes.md"))
        .unwrap()
        .unwrap();

    assert_eq!(
        fixture.service.read(&file.path, "notes.md").unwrap(),
        "notes"
    );
    assert_eq!(
        refusal(fixture.service.read(&file.path, "other.md")),
        Refusal::OutsideCollection
    );
    assert_eq!(
        refusal(fixture.service.create_file(&file.path, "", "x")),
        Refusal::CreateInFolderOnly
    );
    assert_eq!(
        refusal(fixture.service.rename_path(&file.path, "notes.md", "y")),
        Refusal::RenameFromFolder
    );
}

#[test]
fn writes_only_markdown_files() {
    let fixture = fixture();
    let root = root_of(&fixture.repo);
    fixture.service.add(&fixture.repo).unwrap();

    assert_eq!(
        refusal(fixture.service.write(&root, "a.txt", "x")),
        Refusal::MarkdownOnly
    );
    fixture.service.write(&root, "README.md", "new").unwrap();
    assert_eq!(fixture.service.read(&root, "README.md").unwrap(), "new");
}

#[test]
fn creates_a_markdown_file_with_a_valid_name() {
    let fixture = fixture();
    let root = root_of(&fixture.repo);
    fixture.service.add(&fixture.repo).unwrap();

    assert_eq!(
        fixture.service.create_file(&root, "docs", " Api ").unwrap(),
        "docs/Api.md"
    );
    assert_eq!(
        fixture.service.create_file(&root, "", "Top.md").unwrap(),
        "Top.md"
    );
    assert_eq!(
        refusal(fixture.service.create_file(&root, "", "a/b")),
        Refusal::InvalidName
    );
}

#[test]
fn renames_in_place_and_keeps_the_markdown_extension() {
    let fixture = fixture();
    let root = root_of(&fixture.repo);
    fixture.service.add(&fixture.repo).unwrap();

    assert_eq!(
        fixture
            .service
            .rename_path(&root, "docs/guide.md", "Manual")
            .unwrap(),
        "docs/Manual.md"
    );
    assert_eq!(
        fixture
            .service
            .rename_path(&root, "docs", "documentation")
            .unwrap(),
        "documentation"
    );
}

#[test]
fn refuses_to_overwrite_an_existing_entry() {
    let fixture = fixture();
    let root = root_of(&fixture.repo);
    fixture.service.add(&fixture.repo).unwrap();

    fixture
        .files
        .add_file(&fixture.repo.join("docs").join("README.md"), "");

    assert_eq!(
        refusal(
            fixture
                .service
                .rename_path(&root, "docs/guide.md", "../README")
        ),
        Refusal::InvalidName
    );
    assert_eq!(
        refusal(
            fixture
                .service
                .rename_path(&root, "docs/guide.md", "README")
        ),
        Refusal::NameTaken
    );
    assert_eq!(
        refusal(fixture.service.move_path(&root, "README.md", "docs")),
        Refusal::NameTaken
    );
    assert_eq!(
        refusal(fixture.service.move_path(&root, "README.md", "")),
        Refusal::MoveIntoItself
    );
}

#[test]
fn refuses_to_move_a_folder_into_itself() {
    let fixture = fixture();
    let root = root_of(&fixture.repo);
    fixture.service.add(&fixture.repo).unwrap();

    assert_eq!(
        refusal(fixture.service.move_path(&root, "docs", "docs")),
        Refusal::MoveIntoItself
    );
    assert_eq!(
        fixture
            .service
            .move_path(&root, "README.md", "docs")
            .unwrap(),
        "docs/README.md"
    );
}

#[test]
fn trashes_entries_of_the_collection() {
    let fixture = fixture();
    let root = root_of(&fixture.repo);
    fixture.service.add(&fixture.repo).unwrap();

    fixture.service.trash(&root, "docs/guide.md").unwrap();

    let operations = lock(&fixture.files.operations).clone();
    assert_eq!(operations.len(), 1);
    assert!(operations[0].starts_with("trash"));
}

#[test]
fn watches_only_loaded_collections_and_forgets_removed_ones() {
    let fixture = fixture();
    let root = root_of(&fixture.repo);
    fixture.service.add(&fixture.repo).unwrap();

    fixture.service.load(&root).unwrap();
    fixture.service.rewatch_loaded().unwrap();
    fixture.service.remove(&root).unwrap();
    fixture.service.rewatch_loaded().unwrap();

    let watch = format!("watch {root} folder");
    assert_eq!(
        fixture.watcher.calls(),
        [watch.clone(), watch, format!("unwatch {root}")]
    );
}

#[test]
fn numbers_searches_and_skips_empty_ones() {
    let fixture = fixture();

    let first = fixture.service.search("  guide ");
    let second = fixture.service.search("   ");

    assert_eq!((first, second), (1, 2));
    assert_eq!(fixture.watcher.calls(), ["search 1 guide".to_owned()]);
}

#[test]
fn opens_a_file_of_an_open_folder_from_that_folder() {
    let fixture = fixture();
    let root = root_of(&fixture.repo);
    fixture.service.add(&fixture.repo).unwrap();

    let refs = fixture
        .service
        .open_paths(&[
            fixture.repo.join("docs").join("guide.md"),
            base().join("notes.md"),
        ])
        .unwrap();

    let notes = root_of(&base().join("notes.md"));
    assert_eq!(
        refs,
        [
            CollectionFileRef {
                root,
                path: "docs/guide.md".into()
            },
            CollectionFileRef {
                root: notes,
                path: "notes.md".into()
            },
        ]
    );
}

#[test]
fn renames_the_collection_or_restores_its_folder_name() {
    let fixture = fixture();
    let root = root_of(&fixture.repo);
    fixture.service.add(&fixture.repo).unwrap();

    assert_eq!(
        fixture
            .service
            .rename(&root, Some(" Docs "))
            .unwrap()
            .unwrap()
            .display_name,
        "Docs"
    );
    assert_eq!(
        fixture
            .service
            .rename(&root, Some(""))
            .unwrap()
            .unwrap()
            .display_name,
        "repo"
    );
}

#[test]
fn relocates_to_a_new_path_unless_already_open() {
    let fixture = fixture();
    let root = root_of(&fixture.repo);
    fixture.service.add(&fixture.repo).unwrap();
    let moved = base().join("moved");

    assert!(
        fixture
            .service
            .relocate(&root, Some(&fixture.repo))
            .unwrap()
            .is_none()
    );
    assert!(fixture.service.relocate(&root, None).unwrap().is_none());
    assert_eq!(
        fixture
            .service
            .relocate(&root, Some(&moved))
            .unwrap()
            .unwrap()
            .path,
        root_of(&moved)
    );
}
