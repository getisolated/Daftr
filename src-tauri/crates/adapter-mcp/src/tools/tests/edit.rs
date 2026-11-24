//! Édition ciblée, lecture partielle ou groupée, et réponses allégées.

use super::*;

const GUIDE: &str =
    "# Guide\nintro\n## Setup\nstep 1\nstep 2\n### Tips\nbe quick\n## Usage\nrun it\n";

fn edit(id: i64, old: &str, new: &str, replace_all: bool) -> EditNoteArgs {
    EditNoteArgs {
        id,
        old_text: old.to_owned(),
        new_text: new.to_owned(),
        replace_all: Some(replace_all),
    }
}

fn content_of(services: &McpServices, id: i64) -> String {
    get_note(services, &KnownContents::default(), whole(id)).unwrap()["content"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn read(services: &McpServices, id: i64, section: Option<&str>, lines: (u32, u32)) -> ToolResult {
    get_note(
        services,
        &KnownContents::default(),
        GetNoteArgs {
            id,
            section: section.map(str::to_owned),
            from_line: Some(lines.0).filter(|from| *from > 0),
            to_line: Some(lines.1).filter(|to| *to > 0),
        },
    )
}

#[test]
fn edits_one_passage_and_keeps_the_rest() {
    let services = services();
    let id = create(&services, "# Plan\nbuy bread\nbuy milk", &["x"]);

    let edited = edit_note(
        &services,
        &KnownContents::default(),
        edit(id, "bread", "butter", false),
    )
    .unwrap();

    assert_eq!(content_of(&services, id), "# Plan\nbuy butter\nbuy milk");
    assert!(edited.get("content").is_none());
    assert_eq!(edited["tags"], json!(["x"]));
}

#[test]
fn editing_the_first_line_changes_the_title() {
    let services = services();
    let id = create(&services, "# Old\nbody", &[]);

    let edited = edit_note(
        &services,
        &KnownContents::default(),
        edit(id, "# Old", "# New", false),
    )
    .unwrap();

    assert_eq!(edited["title"], "New");
}

#[test]
fn refuses_a_missing_or_ambiguous_passage_without_touching_the_note() {
    let services = services();
    let id = create(&services, "Note\nna na", &[]);

    let missing = edit_note(
        &services,
        &KnownContents::default(),
        edit(id, "zzz", "y", false),
    )
    .unwrap_err();
    let ambiguous = edit_note(
        &services,
        &KnownContents::default(),
        edit(id, "na", "y", false),
    )
    .unwrap_err();
    let empty = edit_note(
        &services,
        &KnownContents::default(),
        edit(id, "", "y", false),
    )
    .unwrap_err();
    let unknown = edit_note(
        &services,
        &KnownContents::default(),
        edit(999, "a", "b", false),
    )
    .unwrap_err();

    assert!(missing.contains("not found"), "{missing}");
    assert!(ambiguous.contains("2 times"), "{ambiguous}");
    assert!(empty.contains("must not be empty"), "{empty}");
    assert!(unknown.contains("Note 999 not found"), "{unknown}");
    assert_eq!(content_of(&services, id), "Note\nna na");
}

#[test]
fn edits_every_occurrence_when_asked() {
    let services = services();
    let id = create(&services, "Note\nna na", &[]);

    edit_note(
        &services,
        &KnownContents::default(),
        edit(id, "na", "ba", true),
    )
    .unwrap();

    assert_eq!(content_of(&services, id), "Note\nba ba");
}

#[test]
fn reads_a_section_with_its_subsections() {
    let services = services();
    let id = create(&services, GUIDE, &[]);

    let section = read(&services, id, Some("setup"), (0, 0)).unwrap();

    assert_eq!(
        section["content"],
        "## Setup\nstep 1\nstep 2\n### Tips\nbe quick\n"
    );
    assert_eq!(section["lines"], "3-7");
    assert_eq!(section["total_lines"], 9);
}

#[test]
fn explains_which_sections_exist() {
    let services = services();
    let id = create(&services, GUIDE, &[]);

    let missing = read(&services, id, Some("Nope"), (0, 0)).unwrap_err();

    assert!(missing.contains("## Setup | ### Tips"), "{missing}");
}

#[test]
fn reads_a_range_of_lines() {
    let services = services();
    let id = create(&services, GUIDE, &[]);

    let part = read(&services, id, None, (4, 5)).unwrap();
    let to_the_end = read(&services, id, None, (8, 0)).unwrap();
    let beyond = read(&services, id, None, (50, 0)).unwrap_err();

    assert_eq!(part["content"], "step 1\nstep 2\n");
    assert_eq!(part["lines"], "4-5");
    assert_eq!(to_the_end["content"], "## Usage\nrun it\n");
    assert!(beyond.contains("9 lines"), "{beyond}");
}

#[test]
fn refuses_a_section_together_with_lines() {
    let services = services();
    let id = create(&services, GUIDE, &[]);

    assert!(read(&services, id, Some("Setup"), (1, 2)).is_err());
}

#[test]
fn reads_several_notes_and_lists_the_missing_ones() {
    let services = services();
    let first = create(&services, "One\na", &[]);
    let second = create(&services, "Two\nb", &[]);

    let both = get_notes(
        &services,
        &KnownContents::default(),
        GetNotesArgs {
            ids: vec![second, first],
        },
    )
    .unwrap();
    let partial = get_notes(
        &services,
        &KnownContents::default(),
        GetNotesArgs {
            ids: vec![first, 999],
        },
    )
    .unwrap();

    assert_eq!(ids(&both), [second, first]);
    assert_eq!(both["notes"][0]["content"], "Two\nb");
    assert!(both.get("missing").is_none());
    assert_eq!(partial["missing"], json!([999]));
    assert!(
        get_notes(
            &services,
            &KnownContents::default(),
            GetNotesArgs { ids: vec![] }
        )
        .is_err()
    );
    assert!(
        get_notes(
            &services,
            &KnownContents::default(),
            GetNotesArgs { ids: vec![1; 21] }
        )
        .is_err()
    );
}

#[test]
fn lists_are_light() {
    let services = services();
    create(&services, "Untagged", &[]);
    create(&services, "Tagged", &["x"]);

    let listed = list_notes(&services, ListNotesArgs::default()).unwrap();
    let untagged = listed["notes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|note| note["title"] == "Untagged")
        .unwrap();

    assert!(untagged.get("created_at").is_none());
    assert!(untagged.get("tags").is_none());
    assert_eq!(untagged["updated_at"].as_str().unwrap().len(), 16);
}

#[test]
fn search_keeps_the_total_beyond_its_limit() {
    let services = services();
    create(&services, "Trip one\nsouth", &[]);
    create(&services, "Trip two\nnorth", &[]);
    create(&services, "Other\nnothing", &[]);
    let args = SearchNotesArgs {
        query: "trip".to_owned(),
        limit: Some(1),
    };

    let found = search_notes(&services, args).unwrap();

    assert_eq!(found["total"], 2);
    assert_eq!(ids(&found).len(), 1);
}

#[test]
fn an_edit_becomes_the_base_of_the_next_update() {
    let services = synced_services();
    let known = KnownContents::default();
    let id = create(&services, "# Plan\nun\ndeux", &[]);

    edit_note(&services, &known, edit(id, "un", "1", false)).unwrap();
    edit_elsewhere(&services, id, "# Plan\n1\ndeux\ntrois");
    let update = UpdateNoteArgs {
        id,
        content: "# Plan\n1\nDEUX".to_owned(),
    };
    update_note(&services, &known, update).unwrap();

    // Le changement fait ailleurs depuis l'édition est gardé
    assert_eq!(content_of(&services, id), "# Plan\n1\nDEUX\ntrois");
}
