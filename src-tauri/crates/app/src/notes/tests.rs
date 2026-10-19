use std::sync::Arc;

use domain::note::{NewNote, NoteChanges};
use platform::sync::lock;

use super::NotesService;
use crate::testing::{MemoryFolders, MemoryNotes, folder, note};

struct Fixture {
    notes: Arc<MemoryNotes>,
    service: NotesService,
}

fn fixture(notes: Vec<domain::note::Note>) -> Fixture {
    let notes = Arc::new(MemoryNotes::with(notes));
    let folders = Arc::new(MemoryFolders::with(vec![
        folder(1, None, true),
        folder(2, None, false),
    ]));
    let service = NotesService::new(notes.clone(), folders);
    Fixture { notes, service }
}

fn calls(fixture: &Fixture) -> Vec<String> {
    lock(&fixture.notes.calls).clone()
}

fn new_note(folder_id: Option<i64>) -> NewNote {
    NewNote {
        title: "t".into(),
        content: String::new(),
        tags: vec![],
        folder_id,
    }
}

#[test]
fn creates_the_note_in_an_existing_folder() {
    let fixture = fixture(vec![]);
    assert_eq!(
        fixture.service.create(new_note(Some(2))).unwrap().folder_id,
        Some(2)
    );
}

#[test]
fn creates_the_note_at_the_root_when_the_folder_is_gone() {
    let fixture = fixture(vec![]);
    assert_eq!(
        fixture
            .service
            .create(new_note(Some(99)))
            .unwrap()
            .folder_id,
        None
    );
}

#[test]
fn an_empty_update_changes_nothing() {
    let fixture = fixture(vec![note(5, None, "[]")]);

    let updated = fixture.service.update(5, &NoteChanges::default()).unwrap();

    assert_eq!(updated.map(|n| n.id), Some(5));
    assert!(
        calls(&fixture)
            .iter()
            .all(|call| !call.starts_with("update"))
    );
}

#[test]
fn moving_to_an_unknown_folder_leaves_the_note_in_place() {
    let fixture = fixture(vec![note(5, Some(2), "[]")]);

    let moved = fixture.service.move_to_folder(5, Some(99)).unwrap();

    assert_eq!(moved.and_then(|n| n.folder_id), Some(2));
    assert!(
        calls(&fixture)
            .iter()
            .all(|call| !call.starts_with("set_folder"))
    );
}

#[test]
fn placing_renumbers_the_target_folder() {
    let fixture = fixture(vec![note(5, None, "[]")]);

    fixture.service.place(5, Some(2), &[7, 5]).unwrap();

    assert!(calls(&fixture).contains(&"set_positions Some(2) [7, 5]".to_owned()));
}

#[test]
fn placing_in_an_unknown_folder_does_not_renumber() {
    let fixture = fixture(vec![note(5, None, "[]")]);

    fixture.service.place(5, Some(99), &[5]).unwrap();

    assert!(
        calls(&fixture)
            .iter()
            .all(|call| !call.starts_with("set_positions"))
    );
}

#[test]
fn normalizes_colors_and_skips_empty_selections() {
    let fixture = fixture(vec![]);

    fixture.service.set_color(&[1], Some("#AABBCC")).unwrap();
    fixture.service.set_color(&[1], Some("red")).unwrap();
    fixture.service.set_color(&[], Some("#aabbcc")).unwrap();

    assert_eq!(
        calls(&fixture),
        [
            "set_color [1] Some(\"#aabbcc\")".to_owned(),
            "set_color [1] None".to_owned(),
        ]
    );
}

#[test]
fn an_empty_search_lists_every_note() {
    let fixture = fixture(vec![]);

    fixture.service.search("   ").unwrap();
    fixture.service.search("c++").unwrap();

    assert_eq!(
        calls(&fixture),
        ["all".to_owned(), "search \"c++\"*".to_owned()]
    );
}

#[test]
fn lists_every_tag_once_sorted() {
    let fixture = fixture(vec![
        note(1, None, "[\"b\",\"a\"]"),
        note(2, None, "[\"a\"]"),
        note(3, None, "oops"),
    ]);
    assert_eq!(fixture.service.all_tags().unwrap(), ["a", "b"]);
}
