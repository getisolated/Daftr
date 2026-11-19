//! Document de note `yrs` (offsets UTF-16) et conversion « contenu complet »
//! → opérations `Y.Text` via `similar`.

use std::time::Duration;

use similar::{DiffTag, TextDiff};
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{
    ClientID, Doc, GetString, OffsetKind, Options, ReadTxn, StateVector, Text, TextRef, Transact,
    Update,
};

/// Au-delà, `similar` rend un diff correct mais moins minimal (gros contenus).
const DIFF_TIMEOUT: Duration = Duration::from_millis(200);

/// Une note : un `Y.Text` nommé `content`.
pub struct NoteDoc {
    doc: Doc,
    text: TextRef,
}

impl NoteDoc {
    /// Document vide, avec un identifiant client stable (dérivé de la clé de l'appareil).
    pub fn new(client_id: u64) -> Self {
        let options = Options {
            offset_kind: OffsetKind::Utf16,
            ..Options::with_client_id(ClientID::new(client_id))
        };
        let doc = Doc::with_options(options);
        let text = doc.get_or_insert_text("content");
        Self { doc, text }
    }

    /// Contenu courant.
    pub fn content(&self) -> String {
        self.text.get_string(&self.doc.transact())
    }

    /// Remplace le contenu par `new` en n'émettant que les opérations minimales.
    pub fn set_content(&self, new: &str) {
        let old = self.content();
        let old_chars: Vec<char> = old.chars().collect();
        let new_chars: Vec<char> = new.chars().collect();
        let diff = TextDiff::configure()
            .timeout(DIFF_TIMEOUT)
            .diff_chars(old.as_str(), new);

        let mut txn = self.doc.transact_mut();
        let mut cursor = 0u32;
        for op in diff.ops() {
            let (tag, old_range, new_range) = op.as_tag_tuple();
            let removed = utf16_len(&old_chars[old_range]);
            let inserted: String = new_chars[new_range].iter().collect();
            match tag {
                DiffTag::Equal => cursor += removed,
                DiffTag::Delete => self.text.remove_range(&mut txn, cursor, removed),
                DiffTag::Insert | DiffTag::Replace => {
                    if tag == DiffTag::Replace {
                        self.text.remove_range(&mut txn, cursor, removed);
                    }
                    self.text.insert(&mut txn, cursor, &inserted);
                    cursor += inserted.encode_utf16().count() as u32;
                }
            }
        }
    }

    /// State vector encodé (v1).
    pub fn state_vector(&self) -> Vec<u8> {
        self.doc.transact().state_vector().encode_v1()
    }

    /// Ce qui manque au pair dont on a reçu le state vector.
    pub fn diff(&self, remote_sv: &[u8]) -> anyhow::Result<Vec<u8>> {
        let sv = StateVector::decode_v1(remote_sv)?;
        Ok(self.doc.transact().encode_diff_v1(&sv))
    }

    /// Applique une mise à jour distante.
    pub fn apply(&self, update: &[u8]) -> anyhow::Result<()> {
        let update = Update::decode_v1(update)?;
        self.doc.transact_mut().apply_update(update)?;
        Ok(())
    }
}

fn utf16_len(chars: &[char]) -> u32 {
    chars.iter().map(|c| c.len_utf16() as u32).sum()
}

#[cfg(test)]
mod tests {
    use super::NoteDoc;

    fn sync(a: &NoteDoc, b: &NoteDoc) {
        let to_b = a.diff(&b.state_vector()).unwrap();
        let to_a = b.diff(&a.state_vector()).unwrap();
        b.apply(&to_b).unwrap();
        a.apply(&to_a).unwrap();
    }

    #[test]
    fn set_content_round_trips_with_utf16_offsets() {
        let doc = NoteDoc::new(1);
        for content in ["", "abc", "é😀\r\nx", "😀😀 ✓ ñ\r\n\r\n", "", "fin 🥚"] {
            doc.set_content(content);
            assert_eq!(doc.content(), content);
        }
    }

    #[test]
    fn concurrent_edits_converge() {
        let a = NoteDoc::new(1);
        let b = NoteDoc::new(2);
        a.set_content("Liste :\n- pain\n- lait\n");
        sync(&a, &b);

        a.set_content("Liste :\n- pain complet\n- lait\n");
        b.set_content("Liste :\r\n- pain\n- lait\n- œufs 🥚\n");
        sync(&a, &b);

        assert_eq!(a.content(), b.content());
        assert_eq!(
            a.content(),
            "Liste :\r\n- pain complet\n- lait\n- œufs 🥚\n"
        );
    }

    #[test]
    fn duplicate_and_reordered_updates_are_idempotent() {
        let a = NoteDoc::new(1);
        let b = NoteDoc::new(2);
        a.set_content("é😀");
        let first = a.diff(&b.state_vector()).unwrap();
        a.set_content("é😀 !");
        let second = a.diff(&b.state_vector()).unwrap();

        b.apply(&second).unwrap();
        b.apply(&first).unwrap();
        b.apply(&second).unwrap();
        assert_eq!(a.content(), b.content());
    }
}
