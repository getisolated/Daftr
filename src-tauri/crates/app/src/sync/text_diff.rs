//! Nouveau contenu complet → opérations minimales sur un `Y.Text`.
//!
//! L'éditeur envoie le texte entier ; on calcule le diff caractère par caractère (`similar`)
//! et on n'applique que les insertions et suppressions, pour que les modifications
//! concurrentes d'autres appareils se fusionnent au lieu d'être écrasées.

use std::time::{Duration, Instant};

use similar::{Algorithm, DiffOp, capture_diff_slices_deadline};
use yrs::{Text, TextRef, TransactionMut};

/// Au-delà, `similar` rend un diff correct mais moins fin plutôt que de bloquer.
const DIFF_TIME_BUDGET: Duration = Duration::from_millis(200);

/// Remplace le texte `old` (contenu actuel de `text`) par `new` avec le moins d'opérations
/// possible. Les positions envoyées à `yrs` sont en unités UTF-16 (document en
/// `OffsetKind::Utf16`).
pub(crate) fn apply_text_diff(
    text: &TextRef,
    transaction: &mut TransactionMut,
    old: &str,
    new: &str,
) {
    let old_chars: Vec<char> = old.chars().collect();
    let new_chars: Vec<char> = new.chars().collect();
    let offsets = utf16_offsets(&old_chars);
    let ops = char_diff(&old_chars, &new_chars);

    // De la fin vers le début : les positions de l'ancien texte restent valables
    for op in ops.iter().rev() {
        apply_op(text, transaction, op, &offsets, &new_chars);
    }
}

/// Diff caractère par caractère de `old` vers `new`, borné dans le temps.
pub(crate) fn char_diff(old: &[char], new: &[char]) -> Vec<DiffOp> {
    let deadline = Instant::now().checked_add(DIFF_TIME_BUDGET);
    capture_diff_slices_deadline(Algorithm::Myers, old, new, deadline)
}

fn apply_op(
    text: &TextRef,
    transaction: &mut TransactionMut,
    op: &DiffOp,
    offsets: &[u32],
    new_chars: &[char],
) {
    match *op {
        DiffOp::Equal { .. } => {}
        DiffOp::Delete {
            old_index, old_len, ..
        } => {
            remove(text, transaction, offsets, old_index, old_len);
        }
        DiffOp::Insert {
            old_index,
            new_index,
            new_len,
        } => {
            let inserted: String = new_chars[new_index..new_index + new_len].iter().collect();
            text.insert(transaction, offsets[old_index], &inserted);
        }
        DiffOp::Replace {
            old_index,
            old_len,
            new_index,
            new_len,
        } => {
            remove(text, transaction, offsets, old_index, old_len);
            let inserted: String = new_chars[new_index..new_index + new_len].iter().collect();
            text.insert(transaction, offsets[old_index], &inserted);
        }
    }
}

fn remove(
    text: &TextRef,
    transaction: &mut TransactionMut,
    offsets: &[u32],
    start: usize,
    len: usize,
) {
    let from = offsets[start];
    text.remove_range(transaction, from, offsets[start + len] - from);
}

/// Position UTF-16 du début de chaque caractère, plus la longueur totale en dernier.
fn utf16_offsets(chars: &[char]) -> Vec<u32> {
    let mut offsets = Vec::with_capacity(chars.len() + 1);
    let mut position = 0_u32;

    offsets.push(position);
    for c in chars {
        // Au plus deux unités par caractère ; le texte tient dans une update de 16 Mio
        position = position.saturating_add(c.len_utf16() as u32);
        offsets.push(position);
    }
    offsets
}

#[cfg(test)]
mod tests {
    use domain::sync::DeviceKey;
    use yrs::{GetString, ReadTxn, Transact};

    use super::*;
    use crate::sync::client::new_doc;

    fn rewrite(old: &str, new: &str) -> (String, u32) {
        let doc = new_doc(&DeviceKey::from_bytes([1; 32]));
        let text = doc.get_or_insert_text("content");
        text.insert(&mut doc.transact_mut(), 0, old);
        let before = doc.transact().state_vector();

        apply_text_diff(&text, &mut doc.transact_mut(), old, new);

        let after = doc.transact().state_vector();
        let client = doc.client_id();
        (
            text.get_string(&doc.transact()),
            after.get(&client) - before.get(&client),
        )
    }

    #[test]
    fn produces_the_new_text() {
        for (old, new) in [
            ("", "abc"),
            ("abc", ""),
            ("Été 🎉 à la plage", "Été 🎉🎉 à la mer"),
            ("a\r\nb\r\n", "a\nb\r\nc"),
            ("👨‍👩‍👧 famille", "👨‍👩 famille !"),
        ] {
            assert_eq!(rewrite(old, new).0, new, "{old:?} → {new:?}");
        }
    }

    #[test]
    fn inserts_only_what_changed() {
        let (text, inserted_units) = rewrite("Bonjour le monde", "Bonjour tout le monde");
        assert_eq!(text, "Bonjour tout le monde");
        assert_eq!(inserted_units, "tout ".len() as u32);
    }

    #[test]
    fn counts_offsets_in_utf16_units() {
        let offsets = utf16_offsets(&['a', '🎉', 'é']);
        assert_eq!(offsets, [0, 1, 3, 4]);
    }
}
