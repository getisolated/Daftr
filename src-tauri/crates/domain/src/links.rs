//! Liens entre notes (`note://…`) : l'identifiant local (entier) d'une note change d'un
//! appareil à l'autre, son uuid non. Les liens écrits avec l'identifiant local sont réécrits
//! avec l'uuid.
//!
//! Les blocs de code (clôturés par ` ``` ` ou `~~~`) et le code en ligne (`` `…` ``, sur une
//! ligne) sont laissés tels quels : un `note://42` y est un exemple, pas un lien. Le renderer
//! continue de résoudre ces anciens liens entiers localement.

#[cfg(test)]
mod tests;

use crate::uuid::Uuid;

/// Préfixe des liens vers une note.
pub const NOTE_LINK_PREFIX: &str = "note://";

/// Longueur de la forme canonique d'un uuid.
const UUID_LENGTH: usize = 36;

/// `content` dont chaque lien `note://<entier>` vers une note connue de `uuid_of` devient
/// `note://<uuid>` ; `None` si rien n'a changé. Idempotente : les liens déjà en uuid et ceux
/// vers une note inconnue restent tels quels.
///
/// ```
/// use domain::links::rewrite_note_links;
/// use domain::uuid::Uuid;
///
/// let uuid = Uuid::from_random([7; 16]);
/// let uuid_of = |id: i64| (id == 4).then_some(uuid);
/// let rewritten = rewrite_note_links("[A](note://4) `note://4`", uuid_of).unwrap();
/// assert_eq!(rewritten, format!("[A](note://{uuid}) `note://4`"));
/// assert_eq!(rewrite_note_links(&rewritten, uuid_of), None);
/// ```
pub fn rewrite_note_links(content: &str, uuid_of: impl Fn(i64) -> Option<Uuid>) -> Option<String> {
    let mut rewritten = String::with_capacity(content.len());
    let mut fence: Option<Fence> = None;
    let mut changed = false;

    for line in content.split_inclusive('\n') {
        match (fence, Fence::open(line)) {
            (Some(open), _) => {
                if open.closes(line) {
                    fence = None;
                }
                rewritten.push_str(line);
            }
            (None, Some(opening)) => {
                fence = Some(opening);
                rewritten.push_str(line);
            }
            (None, None) => changed |= rewrite_line(line, &uuid_of, &mut rewritten),
        }
    }
    changed.then_some(rewritten)
}

/// Ouverture d'un bloc de code clôturé : caractère et longueur de la clôture.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fence {
    marker: char,
    length: usize,
}

impl Fence {
    /// La ligne ouvre un bloc (au plus 3 espaces, puis au moins 3 `` ` `` ou `~`).
    fn open(line: &str) -> Option<Self> {
        let trimmed = indented_at_most_three(line)?;
        let marker = trimmed.chars().next().filter(|c| matches!(c, '`' | '~'))?;
        let length = trimmed.chars().take_while(|c| *c == marker).count();
        let info = &trimmed[length..];

        // Une ligne de backticks suivie d'un autre backtick est du code en ligne
        let valid = length >= 3 && !(marker == '`' && info.contains('`'));
        valid.then_some(Self { marker, length })
    }

    /// La ligne ferme ce bloc : même caractère, au moins aussi long, rien après.
    fn closes(self, line: &str) -> bool {
        let Some(trimmed) = indented_at_most_three(line) else {
            return false;
        };
        let length = trimmed.chars().take_while(|c| *c == self.marker).count();
        length >= self.length && trimmed[length..].trim().is_empty()
    }
}

/// La ligne sans son indentation, si elle est d'au plus 3 espaces.
fn indented_at_most_three(line: &str) -> Option<&str> {
    let trimmed = line.trim_start_matches(' ');
    (line.len() - trimmed.len() <= 3).then_some(trimmed)
}

/// Réécrit les liens d'une ligne hors bloc de code, en sautant le code en ligne. Rend `true`
/// si un lien a changé.
fn rewrite_line(line: &str, uuid_of: &impl Fn(i64) -> Option<Uuid>, out: &mut String) -> bool {
    let mut changed = false;
    let mut rest = line;

    while !rest.is_empty() {
        let code = rest.find('`').unwrap_or(rest.len());
        changed |= rewrite_text(&rest[..code], uuid_of, out);
        rest = &rest[code..];

        let span = inline_code_length(rest);
        out.push_str(&rest[..span]);
        rest = &rest[span..];
    }
    changed
}

/// Longueur du code en ligne au début de `text` (backticks d'ouverture jusqu'à la même suite
/// de fermeture) ; une suite non refermée n'est que du texte : seuls ses backticks comptent.
fn inline_code_length(text: &str) -> usize {
    let ticks = text.chars().take_while(|c| *c == '`').count();
    if ticks == 0 {
        return 0;
    }
    let mut offset = ticks;

    while let Some(start) = text[offset..].find('`') {
        let at = offset + start;
        let run = text[at..].chars().take_while(|c| *c == '`').count();
        if run == ticks {
            return at + run;
        }
        offset = at + run;
    }
    ticks
}

/// Réécrit les liens d'un texte sans code. Rend `true` si un lien a changé.
fn rewrite_text(text: &str, uuid_of: &impl Fn(i64) -> Option<Uuid>, out: &mut String) -> bool {
    let mut changed = false;
    let mut rest = text;

    while let Some(start) = rest.find(NOTE_LINK_PREFIX) {
        let target_start = start + NOTE_LINK_PREFIX.len();
        out.push_str(&rest[..target_start]);
        rest = &rest[target_start..];

        let (consumed, replacement) = link_target(rest, uuid_of);
        match replacement {
            Some(uuid) => {
                out.push_str(&uuid.to_string());
                changed = true;
            }
            None => out.push_str(&rest[..consumed]),
        }
        rest = &rest[consumed..];
    }
    out.push_str(rest);
    changed
}

/// La cible qui suit `note://` : longueur à consommer et uuid de remplacement, `None` pour
/// une cible déjà en uuid, inconnue ou absente.
fn link_target(target: &str, uuid_of: &impl Fn(i64) -> Option<Uuid>) -> (usize, Option<Uuid>) {
    if target.get(..UUID_LENGTH).and_then(Uuid::parse).is_some() {
        return (UUID_LENGTH, None);
    }
    let digits = target.bytes().take_while(u8::is_ascii_digit).count();
    let uuid = target[..digits].parse().ok().and_then(uuid_of);

    (digits, uuid)
}
