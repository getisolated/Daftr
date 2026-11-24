use proptest::prelude::*;

use super::sim::{Replica, exchange, round, settle};

const NAMES: [&str; 4] = ["a.md", "b.md", "docs/c.md", "docs/deep/d.md"];

fn pair() -> (Replica, Replica) {
    (Replica::new(1), Replica::new(2))
}

fn put(replica: &mut Replica, name: &str, text: &str) {
    replica
        .files
        .insert(name.to_owned(), text.as_bytes().to_vec());
}

fn text(replica: &Replica, name: &str) -> Option<String> {
    replica
        .files
        .get(name)
        .map(|bytes| String::from_utf8_lossy(bytes).into_owned())
}

#[test]
fn two_empty_copies_stay_empty() {
    let (mut a, mut b) = pair();
    assert_eq!(settle(&mut a, &mut b), Some(1));
}

#[test]
fn a_new_file_reaches_the_other_copy() {
    let (mut a, mut b) = pair();
    put(&mut a, "docs/new.md", "hello");

    assert!(settle(&mut a, &mut b).is_some());

    assert_eq!(text(&b, "docs/new.md").as_deref(), Some("hello"));
    assert_eq!(a.files, b.files);
}

#[test]
fn successive_saves_never_conflict() {
    let (mut a, mut b) = pair();
    put(&mut a, "n.md", "v1");
    round(&mut a, &mut b, true);
    for version in ["v2", "v3", "v4"] {
        put(&mut a, "n.md", version);
        round(&mut a, &mut b, true);
        put(&mut b, "n.md", &format!("{version}-b"));
        round(&mut a, &mut b, false);
    }
    assert!(settle(&mut a, &mut b).is_some());
    assert_eq!(a.files.len(), 1);
    assert_eq!(text(&a, "n.md").as_deref(), Some("v4-b"));
}

#[test]
fn identical_folders_merge_without_copies() {
    let (mut a, mut b) = pair();
    for replica in [&mut a, &mut b] {
        put(replica, "same.md", "x");
    }
    put(&mut a, "only-a.md", "a");
    put(&mut b, "only-b.md", "b");

    assert!(settle(&mut a, &mut b).is_some());

    let names: Vec<&String> = a.files.keys().collect();
    assert_eq!(names, ["only-a.md", "only-b.md", "same.md"]);
    assert_eq!(a.files, b.files);
}

#[test]
fn differing_files_without_history_are_kept_both() {
    let (mut a, mut b) = pair();
    put(&mut a, "x.md", "from a");
    put(&mut b, "x.md", "from b");

    assert!(settle(&mut a, &mut b).is_some());

    // L'arbitre (la plus petite clé) garde sa version à la place d'origine
    assert_eq!(text(&a, "x.md").as_deref(), Some("from a"));
    assert_eq!(text(&b, "x.md").as_deref(), Some("from a"));
    assert_eq!(text(&a, "x (conflit dev2).md").as_deref(), Some("from b"));
    assert_eq!(a.files, b.files);
}

#[test]
fn a_deletion_goes_to_the_trash_of_the_other_copy() {
    let (mut a, mut b) = pair();
    put(&mut a, "gone.md", "bye");
    assert!(settle(&mut a, &mut b).is_some());

    a.files.remove("gone.md");
    assert!(settle(&mut a, &mut b).is_some());

    assert!(b.files.is_empty());
    assert_eq!(b.trash, vec![b"bye".to_vec()]);
    assert!(a.bases.is_empty() && b.bases.is_empty());
}

#[test]
fn a_modification_wins_over_a_deletion() {
    let (mut a, mut b) = pair();
    put(&mut a, "n.md", "v1");
    assert!(settle(&mut a, &mut b).is_some());

    a.files.remove("n.md");
    put(&mut b, "n.md", "v2");
    assert!(settle(&mut a, &mut b).is_some());

    assert_eq!(text(&a, "n.md").as_deref(), Some("v2"));
    assert_eq!(a.files, b.files);
}

#[test]
fn disjoint_edits_of_a_text_are_merged_identically() {
    let (mut a, mut b) = pair();
    put(&mut a, "n.md", "un\ndeux\ntrois\n");
    assert!(settle(&mut a, &mut b).is_some());

    put(&mut a, "n.md", "UN\ndeux\ntrois\n");
    put(&mut b, "n.md", "un\ndeux\ntrois\nquatre\n");
    assert!(settle(&mut a, &mut b).is_some());

    assert_eq!(
        text(&a, "n.md").as_deref(),
        Some("UN\ndeux\ntrois\nquatre\n")
    );
    assert_eq!(a.files, b.files);
    assert_eq!(a.files.len(), 1);
}

#[test]
fn case_variants_are_never_copied_over_each_other() {
    let (mut a, mut b) = pair();
    put(&mut a, "Notes.md", "a");
    put(&mut b, "notes.md", "b");

    assert!(settle(&mut a, &mut b).is_some());

    assert_eq!(text(&a, "Notes.md").as_deref(), Some("a"));
    assert_eq!(text(&b, "notes.md").as_deref(), Some("b"));
}

#[derive(Debug, Clone)]
enum Op {
    Write {
        on_b: bool,
        file: usize,
        content: u8,
    },
    Delete {
        on_b: bool,
        file: usize,
    },
    Round {
        a_first: bool,
        acks: bool,
    },
}

fn ops() -> impl Strategy<Value = Vec<Op>> {
    let op = prop_oneof![
        (any::<bool>(), 0..NAMES.len(), any::<u8>()).prop_map(|(on_b, file, content)| Op::Write {
            on_b,
            file,
            content
        }),
        (any::<bool>(), 0..NAMES.len()).prop_map(|(on_b, file)| Op::Delete { on_b, file }),
        (any::<bool>(), any::<bool>()).prop_map(|(a_first, acks)| Op::Round { a_first, acks }),
    ];
    prop::collection::vec(op, 1..40)
}

fn apply_user(a: &mut Replica, b: &mut Replica, op: &Op, bytes: impl Fn(u8) -> Vec<u8>) {
    match op {
        Op::Write {
            on_b,
            file,
            content,
        } => {
            let target = if *on_b { b } else { a };
            target
                .files
                .insert(NAMES[*file].to_owned(), bytes(*content));
        }
        Op::Delete { on_b, file } => {
            let target = if *on_b { b } else { a };
            target.files.remove(NAMES[*file]);
        }
        Op::Round { .. } => {}
    }
}

/// Binaire (jamais fusionné) : tout contenu non encore échangé survit à chaque échange.
fn never_loses(ops: &[Op]) -> Result<(), TestCaseError> {
    let (mut a, mut b) = pair();
    for (serial, op) in (1_u32..).zip(ops) {
        let unique = |content: u8| [&[0xff, content][..], &serial.to_le_bytes()].concat();
        apply_user(&mut a, &mut b, op, unique);
        let Op::Round { a_first, acks } = op else {
            continue;
        };
        let mut pending = a.unsynced();
        pending.extend(b.unsynced());
        exchange(&mut a, &mut b, *a_first, *acks);
        let kept: Vec<Vec<u8>> = [a.everything(), b.everything()].concat();
        for content in pending {
            prop_assert!(kept.contains(&content), "contenu perdu : {content:?}");
        }
    }
    prop_assert!(settle(&mut a, &mut b).is_some(), "pas de convergence");
    prop_assert_eq!(&a.files, &b.files);
    Ok(())
}

/// Texte (fusionné au besoin) : les deux copies convergent toujours.
fn always_converges(ops: &[Op]) -> Result<(), TestCaseError> {
    let (mut a, mut b) = pair();
    for op in ops {
        let lines = |content: u8| {
            let words = ["alpha", "beta", "gamma", "delta"];
            let picked = (0..4).filter(|bit| content >> bit & 1 == 1);
            picked
                .map(|bit| format!("{}\n", words[bit]))
                .collect::<String>()
                .into_bytes()
        };
        apply_user(&mut a, &mut b, op, lines);
        if let Op::Round { a_first, acks } = op {
            exchange(&mut a, &mut b, *a_first, *acks);
        }
    }
    prop_assert!(settle(&mut a, &mut b).is_some(), "pas de convergence");
    prop_assert_eq!(&a.files, &b.files);
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(400))]

    #[test]
    fn binary_files_are_never_lost(ops in ops()) {
        never_loses(&ops)?;
    }

    #[test]
    fn text_files_always_converge(ops in ops()) {
        always_converges(&ops)?;
    }
}
