//! Fusion à trois voies : modifications disjointes toutes gardées, chevauchements
//! déterministes, cas triviaux inchangés.

use proptest::prelude::*;

use super::merge;

#[test]
fn without_concurrent_change_the_received_text_wins() {
    assert_eq!(merge("abc", "abc", "abXc"), "abXc");
    assert_eq!(merge("abc", "abc", ""), "");
}

#[test]
fn an_unchanged_received_text_keeps_the_current_one() {
    assert_eq!(merge("abc", "abYc", "abc"), "abYc");
}

#[test]
fn receiving_the_current_text_again_changes_nothing() {
    assert_eq!(
        merge("un deux", "un 2 deux 3", "un 2 deux 3"),
        "un 2 deux 3"
    );
}

#[test]
fn edits_in_different_lines_are_both_kept() {
    let base = "# Titre\nligne A\nligne B\n";
    let current = "# Titre\nligne A\nligne B modifiée ailleurs\n";
    let received = "# Titre modifié ici\nligne A\nligne B\n";

    assert_eq!(
        merge(base, current, received),
        "# Titre modifié ici\nligne A\nligne B modifiée ailleurs\n"
    );
}

#[test]
fn insertions_at_the_same_place_put_the_current_one_first() {
    assert_eq!(merge("ab", "aXb", "aYb"), "aXYb");
}

#[test]
fn identical_insertions_at_the_same_place_are_not_doubled() {
    assert_eq!(merge("ab", "aXbZ", "aXb"), "aXbZ");
}

#[test]
fn a_deletion_keeps_what_the_other_side_inserted_inside() {
    // Le passage « bcd » est supprimé ici ; « X » a été inséré dedans ailleurs
    assert_eq!(merge("abcde", "abcXde", "ae"), "aXe");
}

#[test]
fn a_replacement_keeps_the_other_side_insertions_after_it() {
    assert_eq!(merge("abcde", "abcXde", "aZe"), "aZXe");
}

#[test]
fn deletions_on_both_sides_are_applied_once() {
    assert_eq!(merge("abcdef", "abef", "acdef"), "aef");
}

#[test]
fn counts_characters_not_bytes_or_utf16_units() {
    let base = "é🎉\r\nfin";
    assert_eq!(
        merge(base, "é🎉\r\nfin 👨‍👩‍👧", "Ça é🎉\r\nfin"),
        "Ça é🎉\r\nfin 👨‍👩‍👧"
    );
}

/// Fragments sans `|` : le séparateur borne les zones modifiées.
const FRAGMENTS: &[&str] = &[
    "a",
    "b",
    "é",
    "🎉",
    "👨‍👩‍👧",
    "\r\n",
    "\n",
    " ",
    "Ça",
    "日本",
    "\u{301}",
];

fn text() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(FRAGMENTS), 0..16).prop_map(|parts| parts.concat())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    /// Une zone modifiée ici, une autre ailleurs (séparées par `|…|`) : les deux sont gardées,
    /// quel que soit le côté qui modifie le début.
    #[test]
    fn disjoint_local_and_remote_edits_are_both_kept(
        start in text(),
        middle in text(),
        end in text(),
        new_start in text(),
        new_end in text(),
        local_first in any::<bool>(),
    ) {
        let base = format!("{start}|{middle}|{end}");
        let edited_start = format!("{new_start}|{middle}|{end}");
        let edited_end = format!("{start}|{middle}|{new_end}");
        let (current, received) = if local_first {
            (edited_end, edited_start)
        } else {
            (edited_start, edited_end)
        };

        prop_assert_eq!(
            merge(&base, &current, &received),
            format!("{new_start}|{middle}|{new_end}")
        );
    }

    #[test]
    fn same_base_and_current_gives_the_received_text(base in text(), received in text()) {
        prop_assert_eq!(merge(&base, &base, &received), received);
    }

    #[test]
    fn receiving_the_current_text_is_idempotent(base in text(), current in text()) {
        prop_assert_eq!(merge(&base, &current, &current), current);
    }
}
