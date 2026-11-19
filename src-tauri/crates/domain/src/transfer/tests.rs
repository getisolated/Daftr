use super::*;

fn folder(id: i64, parent_id: Option<i64>) -> Folder {
    Folder {
        id,
        name: format!("folder {id}"),
        parent_id,
        is_default: false,
        sort_order: Some(id),
        color: (id == 2).then(|| "#112233".to_owned()),
        created_at: format!("2024-01-0{id} 10:00:00"),
    }
}

fn note(id: i64, folder_id: Option<i64>) -> Note {
    Note {
        id,
        title: format!("note {id}"),
        content: format!("# note {id}"),
        tags: "[\"a\"]".to_owned(),
        folder_id,
        sort_order: Some(1),
        color: None,
        created_at: "2023-05-01 08:00:00".to_owned(),
        updated_at: "2023-06-01 09:00:00".to_owned(),
    }
}

/// Inbox (1, par défaut) ; Projets (2) > Client (3) > Archives (4) ; Perso (5).
fn tree() -> (Vec<Folder>, Vec<Note>) {
    let mut inbox = folder(1, None);
    inbox.is_default = true;
    let folders = vec![
        inbox,
        folder(2, None),
        folder(3, Some(2)),
        folder(4, Some(3)),
        folder(5, None),
    ];
    let notes = vec![
        note(10, Some(1)),
        note(11, Some(2)),
        note(12, Some(3)),
        note(13, Some(4)),
        note(14, Some(5)),
        note(15, None),
    ];
    (folders, notes)
}

#[test]
fn moves_a_folder_with_its_whole_subtree_in_parent_first_order() {
    let (folders, notes) = tree();

    let plan = plan_transfer(&folders, &notes, &[2], &[]);

    let keys: Vec<(i64, Option<i64>)> = plan
        .bundle
        .folders
        .iter()
        .map(|f| (f.key, f.parent_key))
        .collect();
    assert_eq!(keys, [(2, None), (3, Some(2)), (4, Some(3))]);
    let note_folders: Vec<Option<i64>> = plan.bundle.notes.iter().map(|n| n.folder_key).collect();
    assert_eq!(note_folders, [Some(2), Some(3), Some(4)]);
    assert_eq!(plan.root_folders, [2]);
    assert_eq!(plan.moved_folders, [2, 3, 4]);
    assert_eq!(plan.moved_notes, [11, 12, 13]);
    assert!(plan.loose_notes.is_empty());
}

#[test]
fn keeps_the_content_dates_and_colors_but_not_the_position_of_a_moved_root() {
    let (folders, notes) = tree();

    let plan = plan_transfer(&folders, &notes, &[2], &[]);

    let root = &plan.bundle.folders[0];
    assert_eq!(root.color.as_deref(), Some("#112233"));
    assert_eq!(root.created_at, "2024-01-02 10:00:00");
    assert_eq!(root.sort_order, None);
    assert_eq!(plan.bundle.folders[1].sort_order, Some(3));

    let copied = &plan.bundle.notes[0];
    assert_eq!(copied.content, "# note 11");
    assert_eq!(copied.tags, "[\"a\"]");
    assert_eq!(copied.created_at, "2023-05-01 08:00:00");
    assert_eq!(copied.updated_at, "2023-06-01 09:00:00");
    assert_eq!(copied.sort_order, Some(1));
}

#[test]
fn a_loose_note_goes_to_the_default_folder_of_the_target() {
    let (folders, notes) = tree();

    let plan = plan_transfer(&folders, &notes, &[], &[12, 15]);

    let note_folders: Vec<Option<i64>> = plan.bundle.notes.iter().map(|n| n.folder_key).collect();
    assert_eq!(note_folders, [None, None]);
    assert!(plan.bundle.notes.iter().all(|n| n.sort_order.is_none()));
    assert_eq!(plan.loose_notes, [12, 15]);
    assert_eq!(plan.moved_notes, [12, 15]);
    assert!(plan.bundle.folders.is_empty());
}

#[test]
fn does_not_move_twice_what_a_moved_folder_already_holds() {
    let (folders, notes) = tree();

    let plan = plan_transfer(&folders, &notes, &[3, 2, 4, 2], &[13, 14, 14]);

    assert_eq!(plan.root_folders, [2]);
    assert_eq!(plan.moved_folders, [2, 3, 4]);
    assert_eq!(plan.loose_notes, [14]);
    assert_eq!(plan.moved_notes, [11, 12, 13, 14]);
    assert_eq!(plan.bundle.notes.len(), 4);
}

#[test]
fn ignores_the_default_folder_and_unknown_items() {
    let (folders, notes) = tree();

    let plan = plan_transfer(&folders, &notes, &[1, 99], &[98]);

    assert!(plan.bundle.is_empty());
    assert_eq!(plan, TransferPlan::default());
}

#[test]
fn survives_a_cycle_of_parents() {
    let folders = vec![folder(2, Some(3)), folder(3, Some(2))];

    let plan = plan_transfer(&folders, &[], &[2], &[]);

    assert_eq!(plan.moved_folders, [2, 3]);
}
