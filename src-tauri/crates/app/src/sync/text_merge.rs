//! Fusion à trois voies d'un texte : un contenu construit sur une base ancienne est rejoué
//! sur le texte courant, sans effacer ce qui a changé entre-temps (modification d'un autre
//! appareil appliquée pendant qu'un enregistrement était en vol).
//!
//! Les deux diffs partent de la base : chacun dit, pour chaque caractère de la base, s'il est
//! gardé et ce qui est inséré avant lui. Le résultat les tisse ensemble, avec les règles de
//! [`merge`].

#[cfg(test)]
mod tests;

use std::ops::Range;

use similar::DiffOp;

use crate::sync::text_diff::char_diff;

/// `received`, construit sur `base`, rejoué sur `current` (le texte actuel, qui a pu
/// recevoir d'autres modifications depuis `base`).
///
/// Règles, déterministes :
/// - un caractère de la base supprimé d'un côté est supprimé du résultat ;
/// - toutes les insertions sont gardées ; à la même position, celle de `current` d'abord,
///   puis celle de `received`, et deux insertions identiques n'en font qu'une ;
/// - un passage remplacé d'un côté et modifié de l'autre garde le remplacement et les
///   insertions de l'autre côté (comme la fusion du CRDT), sans marqueur de conflit.
///
/// ```
/// use app::sync::text_merge::merge;
///
/// let base = "un\ndeux\ntrois";
/// let current = "un\ndeux\ntrois !";
/// let received = "UN\ndeux\ntrois";
/// assert_eq!(merge(base, current, received), "UN\ndeux\ntrois !");
/// ```
pub fn merge(base: &str, current: &str, received: &str) -> String {
    if current == base || received == current {
        return received.to_owned();
    }
    if received == base {
        return current.to_owned();
    }
    let base: Vec<char> = base.chars().collect();
    let current: Vec<char> = current.chars().collect();
    let received: Vec<char> = received.chars().collect();
    let theirs = SideEdits::between(&base, &current);
    let ours = SideEdits::between(&base, &received);

    weave(&base, theirs, ours)
}

/// Ce qu'un côté a fait de la base : passages supprimés et insertions, triés.
struct SideEdits<'a> {
    text: &'a [char],
    /// Position dans la base (insertion avant ce caractère) et texte inséré dans `text`.
    inserts: Vec<(usize, Range<usize>)>,
    deleted: Vec<Range<usize>>,
}

impl<'a> SideEdits<'a> {
    fn between(base: &[char], text: &'a [char]) -> Self {
        let mut edits = Self {
            text,
            inserts: Vec::new(),
            deleted: Vec::new(),
        };
        for op in char_diff(base, text) {
            edits.record(op);
        }
        edits
    }

    fn record(&mut self, op: DiffOp) {
        match op {
            DiffOp::Equal { .. } => {}
            DiffOp::Delete {
                old_index, old_len, ..
            } => self.deleted.push(old_index..old_index + old_len),
            DiffOp::Insert {
                old_index,
                new_index,
                new_len,
            } => self
                .inserts
                .push((old_index, new_index..new_index + new_len)),
            DiffOp::Replace {
                old_index,
                old_len,
                new_index,
                new_len,
            } => {
                self.deleted.push(old_index..old_index + old_len);
                self.inserts
                    .push((old_index, new_index..new_index + new_len));
            }
        }
    }
}

/// Parcours d'un côté, position après position de la base.
struct Cursor<'a> {
    edits: SideEdits<'a>,
    next_insert: usize,
    next_deleted: usize,
}

impl<'a> Cursor<'a> {
    fn new(edits: SideEdits<'a>) -> Self {
        Self {
            edits,
            next_insert: 0,
            next_deleted: 0,
        }
    }

    /// Ce que ce côté insère avant le caractère `position` de la base.
    fn inserted_at(&mut self, position: usize) -> &'a [char] {
        let text = self.edits.text;
        let first = self.next_insert;
        while self
            .edits
            .inserts
            .get(self.next_insert)
            .is_some_and(|(at, _)| *at == position)
        {
            self.next_insert += 1;
        }
        // Les insertions d'un même diff à la même position sont contiguës dans `text`
        let mut ranges = self.edits.inserts[first..self.next_insert].iter();
        match (ranges.next(), ranges.next_back()) {
            (None, _) => &[],
            (Some((_, start)), last) => {
                let end = last.map_or(start.end, |(_, range)| range.end);
                &text[start.start..end]
            }
        }
    }

    /// Ce côté a supprimé le caractère `position` de la base (positions croissantes).
    fn deletes(&mut self, position: usize) -> bool {
        let deleted = &self.edits.deleted;
        while deleted
            .get(self.next_deleted)
            .is_some_and(|range| range.end <= position)
        {
            self.next_deleted += 1;
        }
        deleted
            .get(self.next_deleted)
            .is_some_and(|range| range.start <= position)
    }
}

fn weave(base: &[char], theirs: SideEdits<'_>, ours: SideEdits<'_>) -> String {
    let mut merged = String::with_capacity(theirs.text.len().max(ours.text.len()) * 2);
    let mut theirs = Cursor::new(theirs);
    let mut ours = Cursor::new(ours);

    for position in 0..=base.len() {
        let their_insert = theirs.inserted_at(position);
        let our_insert = ours.inserted_at(position);
        merged.extend(their_insert);
        if our_insert != their_insert {
            merged.extend(our_insert);
        }

        let Some(&kept) = base.get(position) else {
            break;
        };
        // Les deux curseurs avancent : pas de court-circuit
        let deleted = [theirs.deletes(position), ours.deletes(position)];
        if !deleted.contains(&true) {
            merged.push(kept);
        }
    }
    merged
}
