use std::collections::HashMap;

use super::*;

/// `(hash, base)` d'un côté ; `None` : fichier absent (supprimé si une base existe).
type Side<'a> = (Option<&'a str>, Option<&'a str>);

fn manifest_of(files: &[(&str, Side)]) -> Manifest {
    let mut entries = Vec::new();
    for (name, (current, base)) in files {
        entries.push(FileEntry {
            path: path(name),
            hash: current.map(hash),
            size: 1,
            base: base.map(hash),
        });
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    Manifest {
        entries,
        ignored: Vec::new(),
    }
}

/// L'action du côté local pour un fichier, `None` : rien à faire.
fn action(local: Side, remote: Side) -> Option<FileAction> {
    let mine = manifest_of(&[("f.md", local)]);
    let theirs = manifest_of(&[("f.md", remote)]);
    plan(&mine, &theirs.entries)
        .into_iter()
        .next()
        .map(|planned| planned.action)
}

#[test]
fn identical_files_only_record_the_base() {
    assert_eq!(
        action((Some("a"), None), (Some("a"), None)),
        Some(FileAction::Adopt)
    );
    assert_eq!(action((Some("a"), Some("a")), (Some("a"), Some("a"))), None);
    assert_eq!(
        action((None, Some("a")), (None, Some("a"))),
        Some(FileAction::Forget)
    );
    assert_eq!(action((None, None), (None, None)), None);
}

#[test]
fn a_file_known_by_one_side_only_is_received_without_history() {
    assert_eq!(
        action((None, None), (Some("a"), None)),
        Some(FileAction::Pull)
    );
    assert_eq!(action((Some("a"), None), (None, None)), None);
    assert_eq!(
        action((Some("a"), None), (Some("b"), None)),
        Some(FileAction::Reconcile)
    );
}

#[test]
fn a_change_on_one_side_goes_to_the_other() {
    let base = Some("v1");
    assert_eq!(
        action((base, base), (Some("v2"), base)),
        Some(FileAction::Pull)
    );
    assert_eq!(action((Some("v2"), base), (base, base)), None);
}

#[test]
fn a_deletion_goes_to_the_untouched_side() {
    let base = Some("v1");
    assert_eq!(
        action((base, base), (None, base)),
        Some(FileAction::DeleteLocal)
    );
    assert_eq!(action((None, base), (base, base)), None);
}

#[test]
fn a_modification_wins_over_a_deletion() {
    let base = Some("v1");
    assert_eq!(
        action((None, base), (Some("v2"), base)),
        Some(FileAction::Pull)
    );
    assert_eq!(action((Some("v2"), base), (None, base)), None);
}

#[test]
fn both_sides_changing_is_a_conflict() {
    let base = Some("v1");
    let both = action((Some("v2"), base), (Some("v3"), base));
    assert_eq!(both, Some(FileAction::Reconcile));
}

#[test]
fn a_base_known_by_one_side_only_is_ignored() {
    // Sans base commune, une suppression d'un côté se lit comme une absence : l'autre côté
    // garde son fichier (jamais de perte), et deux versions différentes sont en conflit.
    assert_eq!(action((Some("v1"), Some("v1")), (None, None)), None);
    assert_eq!(
        action((None, None), (Some("v1"), Some("v1"))),
        Some(FileAction::Pull)
    );
    let differing = action((Some("v2"), Some("old")), (Some("v3"), Some("v1")));
    assert_eq!(differing, Some(FileAction::Reconcile));
    // Local inchangé selon sa base, mais l'autre côté a une autre base : pas de réception
    // aveugle qui pourrait échanger les deux versions.
    let stale = action((Some("v1"), Some("v1")), (Some("v2"), Some("v0")));
    assert_eq!(stale, Some(FileAction::Reconcile));
}

#[test]
fn the_plan_carries_what_the_execution_checks() {
    let base = Some("v1");
    let mine = manifest_of(&[("f.md", (Some("v2"), base))]);
    let theirs = manifest_of(&[("f.md", (Some("v3"), base))]);

    let planned = plan(&mine, &theirs.entries);

    assert_eq!(planned.len(), 1);
    assert_eq!(planned[0].local, Some(hash("v2")));
    assert_eq!(planned[0].remote, Some(hash("v3")));
    assert_eq!(planned[0].base, Some(hash("v1")));
}

#[test]
fn files_only_on_the_other_side_are_pulled_in_path_order() {
    let mine = manifest_of(&[("keep.md", (Some("k"), Some("k")))]);
    let theirs = manifest_of(&[
        ("keep.md", (Some("k"), Some("k"))),
        ("z.md", (Some("z"), None)),
        ("docs/a.md", (Some("a"), None)),
    ]);

    let planned = plan(&mine, &theirs.entries);

    let names: Vec<&str> = planned.iter().map(|item| item.path.as_str()).collect();
    assert_eq!(names, ["docs/a.md", "z.md"]);
    assert!(planned.iter().all(|item| item.action == FileAction::Pull));
}

#[test]
fn case_variants_are_skipped_not_overwritten() {
    let mine = manifest_of(&[("Notes.md", (Some("a"), None))]);
    let theirs = manifest_of(&[("notes.md", (Some("b"), None))]);

    let planned = plan(&mine, &theirs.entries);

    assert_eq!(planned.len(), 1);
    assert_eq!(
        planned[0].action,
        FileAction::Skip(IgnoredReason::CaseCollision)
    );
}

#[test]
fn a_remote_collision_skips_both_variants() {
    let mine = Manifest::default();
    let theirs = manifest_of(&[("A.md", (Some("a"), None)), ("a.md", (Some("b"), None))]);

    let planned = plan(&mine, &theirs.entries);

    assert_eq!(planned.len(), 2);
    assert!(
        planned
            .iter()
            .all(|item| item.action == FileAction::Skip(IgnoredReason::CaseCollision))
    );
}

#[test]
fn an_oversized_remote_file_is_skipped() {
    let mine = Manifest::default();
    let mut theirs = manifest_of(&[("big.md", (Some("x"), None))]);
    theirs.entries[0].size = MAX_FILE_BYTES + 1;

    let planned = plan(&mine, &theirs.entries);

    assert_eq!(planned[0].action, FileAction::Skip(IgnoredReason::TooLarge));
}

#[test]
fn the_manifest_lists_deletions_and_drops_case_collisions() {
    let files = vec![
        LocalFile {
            path: path("a.md"),
            hash: hash("a"),
            size: 1,
        },
        LocalFile {
            path: path("B.md"),
            hash: hash("b"),
            size: 1,
        },
        LocalFile {
            path: path("b.md"),
            hash: hash("b2"),
            size: 2,
        },
    ];
    let bases = HashMap::from([(path("a.md"), hash("old")), (path("gone.md"), hash("g"))]);

    let manifest = build_manifest(files, &bases);

    let names: Vec<&str> = manifest
        .entries
        .iter()
        .map(|entry| entry.path.as_str())
        .collect();
    assert_eq!(names, ["a.md", "gone.md"]);
    assert_eq!(manifest.entries[0].base, Some(hash("old")));
    assert_eq!(manifest.entries[1].hash, None);
    assert_eq!(manifest.entries[1].base, Some(hash("g")));
    assert_eq!(manifest.ignored.len(), 2);
    assert!(
        manifest
            .ignored
            .iter()
            .all(|i| i.reason == IgnoredReason::CaseCollision)
    );
}

#[test]
fn roles_are_opposite_on_both_sides() {
    assert_eq!(Role::between(&1, &2), Role::Arbiter);
    assert_eq!(Role::between(&2, &1), Role::Follower);
}

#[test]
fn a_conflict_copy_is_named_after_the_device_and_never_overwrites() {
    let name = conflict_name(&path("docs/plan.md"), "PC-Portable", |_| false);
    assert_eq!(name.as_str(), "docs/plan (conflit PC-Portable).md");

    let taken = |candidate: &str| candidate == "docs/plan (conflit PC-Portable).md";
    let second = conflict_name(&path("docs/plan.md"), "PC-Portable", taken);
    assert_eq!(second.as_str(), "docs/plan (conflit PC-Portable 2).md");

    let upper = conflict_name(&path("PLAN.MD"), "Bureau", |_| false);
    assert_eq!(upper.as_str(), "PLAN (conflit Bureau).MD");
}

#[test]
fn a_device_name_is_cleaned_for_a_file_name() {
    let name = conflict_name(&path("a.md"), "PC: <1> (x)/..", |_| false);
    assert_eq!(name.as_str(), "a (conflit PC_ _1_ _x__).md");
    let blank = conflict_name(&path("a.md"), " . ", |_| false);
    assert_eq!(blank.as_str(), "a (conflit appareil).md");
}

#[test]
fn a_conflict_copy_is_recognized_by_its_name() {
    let parsed = parse_conflict("docs/plan (conflit PC-Portable 2).md").unwrap();
    assert_eq!(parsed.original, "docs/plan.md");
    assert!(parsed.is_from("PC-Portable"));
    assert!(!parsed.is_from("Bureau"));
    assert!(parse_conflict("docs/plan.md").is_none());
    assert!(parse_conflict("(conflit x).md").is_none());
    assert!(parse_conflict("a (conflit ).md").is_none());
}

#[test]
fn a_clean_merge_is_the_same_on_both_sides() {
    let base = b"un\ndeux\ntrois";
    let mine = b"UN\ndeux\ntrois";
    let theirs = b"un\ndeux\ntrois !";

    let on_arbiter = resolve(Role::Arbiter, Some(base), mine, theirs);
    let on_follower = resolve(Role::Follower, Some(base), theirs, mine);

    assert_eq!(on_arbiter, on_follower);
    assert_eq!(
        on_arbiter,
        Resolution::Merged(b"UN\ndeux\ntrois !".to_vec())
    );
}

#[test]
fn concurrent_insertions_at_one_spot_merge_in_the_same_order_everywhere() {
    let base = b"a\nz";
    let left = b"a\nleft\nz";
    let right = b"a\nright\nz";

    let on_arbiter = resolve(Role::Arbiter, Some(base), left, right);
    let on_follower = resolve(Role::Follower, Some(base), right, left);

    assert_eq!(on_arbiter, on_follower);
}

#[test]
fn without_a_base_or_a_text_both_versions_are_kept() {
    assert_eq!(
        resolve(Role::Arbiter, None, b"a", b"b"),
        Resolution::KeepBoth
    );
    let binary = [0xff, 0xfe, 0x00];
    assert_eq!(
        resolve(Role::Arbiter, Some(b"a"), &binary, b"b"),
        Resolution::KeepBoth
    );
    assert_eq!(
        resolve(Role::Follower, Some(&binary), b"a", b"b"),
        Resolution::KeepBoth
    );
}
