//! Convergence : des appareils qui modifient la même note en parallèle et s'échangent leurs
//! diffs dans n'importe quel ordre, avec des doublons et des updates périmées, finissent
//! avec la même projection.

use proptest::prelude::*;

use super::content::FRAGMENTS;
use super::*;
use crate::sync::note_doc::{apply_color, apply_delete, apply_placement, apply_restore, project};

const DEVICES: u8 = 4;

#[derive(Debug, Clone)]
enum Action {
    /// Insère `insert` à la position `at` et retire `remove` caractères après.
    Edit {
        device: u8,
        at: usize,
        remove: usize,
        insert: String,
    },
    /// `from` envoie son diff à `to`.
    Send {
        from: u8,
        to: u8,
    },
    /// Une update déjà envoyée est rejouée sur un appareil (doublon ou update périmée).
    Replay {
        device: u8,
        index: usize,
    },
    Delete {
        device: u8,
    },
    Restore {
        device: u8,
    },
    Move {
        device: u8,
        folder: u8,
        order: i64,
    },
    Color {
        device: u8,
        color: u8,
    },
}

fn device_index() -> impl Strategy<Value = u8> {
    0..DEVICES
}

fn action() -> impl Strategy<Value = Action> {
    let insert = prop::collection::vec(prop::sample::select(FRAGMENTS), 0..4)
        .prop_map(|parts| parts.concat());
    prop_oneof![
        4 => (device_index(), 0..64_usize, 0..4_usize, insert)
            .prop_map(|(device, at, remove, insert)| Action::Edit { device, at, remove, insert }),
        4 => (device_index(), device_index()).prop_map(|(from, to)| Action::Send { from, to }),
        1 => (device_index(), 0..64_usize)
            .prop_map(|(device, index)| Action::Replay { device, index }),
        1 => device_index().prop_map(|device| Action::Delete { device }),
        1 => device_index().prop_map(|device| Action::Restore { device }),
        1 => (device_index(), 0..3_u8, 0..3_i64)
            .prop_map(|(device, folder, order)| Action::Move { device, folder, order }),
        1 => (device_index(), 0..3_u8).prop_map(|(device, color)| Action::Color { device, color }),
    ]
}

/// Le contenu de la note avec une modification à des positions de caractères (jamais au
/// milieu d'un caractère).
fn edited(current: &str, at: usize, remove: usize, insert: &str) -> String {
    let chars: Vec<char> = current.chars().collect();
    let start = at.min(chars.len());
    let end = (start + remove).min(chars.len());

    let mut next: String = chars[..start].iter().collect();
    next.push_str(insert);
    next.extend(&chars[end..]);
    next
}

fn play(docs: &[NoteDoc], sent: &mut Vec<Update>, action: &Action) {
    match action {
        Action::Edit {
            device,
            at,
            remove,
            insert,
        } => {
            let doc = &docs[usize::from(*device)];
            edit(doc, &edited(&content(doc), *at, *remove, insert));
        }
        Action::Send { from, to } => {
            sent.push(send(&docs[usize::from(*from)], &docs[usize::from(*to)]));
        }
        Action::Replay { device, index } if !sent.is_empty() => {
            let update = &sent[index % sent.len()];
            docs[usize::from(*device)].apply_update(update).unwrap();
        }
        Action::Replay { .. } => {}
        Action::Delete { device } => {
            apply_delete(&docs[usize::from(*device)], LATER);
        }
        Action::Restore { device } => {
            apply_restore(&docs[usize::from(*device)]);
        }
        Action::Move {
            device,
            folder,
            order,
        } => {
            let folder = Some(Uuid::from_bytes([*folder; 16]));
            apply_placement(&docs[usize::from(*device)], folder, Some(*order));
        }
        Action::Color { device, color } => {
            let color = ["#112233", "#aabbcc"].get(usize::from(*color)).copied();
            apply_color(&docs[usize::from(*device)], color);
        }
    }
}

/// Tous les appareils finissent par tout se transmettre.
fn settle(docs: &[NoteDoc]) {
    for _ in 0..2 {
        for from in docs {
            for to in docs {
                send(from, to);
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn devices_converge_to_the_same_projection(
        actions in prop::collection::vec(action(), 0..48),
    ) {
        let docs = shared_note(DEVICES, "Début é 🎉\r\nfin");
        let mut sent = Vec::new();
        for action in &actions {
            play(&docs, &mut sent, action);
        }
        settle(&docs);

        let expected = project(&docs[0], note_uuid());
        for doc in &docs[1..] {
            prop_assert!(!doc.has_missing_updates());
            prop_assert_eq!(&project(doc, note_uuid()), &expected);
        }
    }
}
