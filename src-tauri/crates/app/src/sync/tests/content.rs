//! Le contenu projeté est exactement celui écrit, quels que soient les caractères.

use proptest::prelude::*;

use super::*;
use crate::sync::note_doc::{apply_changes, from_note, project};

/// Morceaux de texte qui piègent les indices : accents, emojis (paires de substitution
/// UTF-16), séquences ZWJ, fins de ligne CRLF.
pub(super) const FRAGMENTS: &[&str] = &[
    "a",
    "é",
    "ü",
    "🎉",
    "👨‍👩‍👧",
    "\r\n",
    "\n",
    " ",
    "Ça",
    "日本",
    "ﬁ",
    "\u{301}",
];

pub(super) fn text() -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(FRAGMENTS), 0..24).prop_map(|parts| parts.concat())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn projects_exactly_the_written_content(initial in text(), target in text()) {
        let doc = from_note(&device(0), &note_fields(&initial));
        let changes = NoteEdit { content: Some(&target), tags: None };
        apply_changes(&doc, &changes, LATER);

        prop_assert_eq!(&project(&doc, note_uuid()).content, &target);
        let reloaded = NoteDoc::load(&device(0), &doc.encode()).unwrap();
        prop_assert_eq!(project(&reloaded, note_uuid()).content, target);
    }

    #[test]
    fn projects_any_unicode_content(initial in any::<String>(), target in any::<String>()) {
        let doc = from_note(&device(0), &note_fields(&initial));
        edit(&doc, &target);
        prop_assert_eq!(content(&doc), target);
    }
}
