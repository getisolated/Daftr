//! Édition ciblée du texte d'une note : remplacer un passage, lire une section ou des lignes.
//!
//! Fonctions pures, sans I/O : elles évitent de relire et de réécrire toute la note pour en
//! changer ou en lire une partie.

/// Pourquoi un passage n'a pas pu être remplacé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplaceError {
    /// Le passage à remplacer est vide.
    EmptyPassage,
    /// Le passage n'apparaît pas dans le texte.
    NotFound,
    /// Le passage apparaît plusieurs fois (le nombre d'occurrences) alors qu'une seule
    /// était attendue.
    Ambiguous(usize),
}

/// Un titre markdown (`# titre`) du texte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    /// Niveau, de 1 (`#`) à 6.
    pub level: usize,
    /// Texte du titre, sans les `#`.
    pub text: String,
    /// Ligne du titre, à partir de 1.
    pub line: usize,
}

/// `content` où `old` est remplacé par `new`. Sans `replace_all`, `old` doit être unique :
/// une occurrence ambiguë est refusée plutôt que de modifier le mauvais endroit.
///
/// ```
/// use domain::note::edit::replace_passage;
///
/// assert_eq!(replace_passage("a b a", "b", "c", false).unwrap(), "a c a");
/// assert!(replace_passage("a b a", "a", "c", false).is_err());
/// assert_eq!(replace_passage("a b a", "a", "c", true).unwrap(), "c b c");
/// ```
pub fn replace_passage(
    content: &str,
    old: &str,
    new: &str,
    replace_all: bool,
) -> Result<String, ReplaceError> {
    if old.is_empty() {
        return Err(ReplaceError::EmptyPassage);
    }
    let (old, new) = in_line_endings_of(content, old, new);
    let occurrences = content.matches(old.as_str()).count();

    match occurrences {
        0 => Err(ReplaceError::NotFound),
        1 => Ok(content.replacen(old.as_str(), &new, 1)),
        _ if replace_all => Ok(content.replace(old.as_str(), &new)),
        _ => Err(ReplaceError::Ambiguous(occurrences)),
    }
}

/// Nombre de lignes du texte (0 s'il est vide).
pub fn line_count(content: &str) -> usize {
    content.lines().count()
}

/// Les lignes `from` à `to` (à partir de 1, bornes comprises, ramenées au texte).
///
/// ```
/// assert_eq!(domain::note::edit::lines_between("a\nb\nc\n", 2, 3), "b\nc\n");
/// ```
pub fn lines_between(content: &str, from: usize, to: usize) -> String {
    content
        .split_inclusive('\n')
        .skip(from.saturating_sub(1))
        .take((to + 1).saturating_sub(from.max(1)))
        .collect()
}

/// Les titres du texte, hors blocs de code.
pub fn headings(content: &str) -> Vec<Heading> {
    let mut in_code = false;
    let mut found = Vec::new();

    for (index, line) in content.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            in_code = !in_code;
        } else if !in_code && let Some((level, text)) = parse_heading(line) {
            found.push(Heading {
                level,
                text,
                line: index + 1,
            });
        }
    }
    found
}

/// Lignes (début, fin, à partir de 1) de la section dont le titre est `title` (casse et `#`
/// ignorés), sous-sections comprises : elle court jusqu'au prochain titre de même niveau ou
/// plus haut. S'il y a plusieurs titres identiques, c'est le premier.
///
/// ```
/// let text = "# A\nx\n## B\ny\n# C\nz\n";
/// assert_eq!(domain::note::edit::section_lines(text, "a"), Some((1, 4)));
/// assert_eq!(domain::note::edit::section_lines(text, "## B"), Some((3, 4)));
/// ```
pub fn section_lines(content: &str, title: &str) -> Option<(usize, usize)> {
    let wanted = title.trim_start_matches('#').trim().to_lowercase();
    let all = headings(content);
    let position = all
        .iter()
        .position(|heading| heading.text.to_lowercase() == wanted)?;
    let start = &all[position];
    let end = all[position + 1..]
        .iter()
        .find(|heading| heading.level <= start.level)
        .map_or_else(|| line_count(content), |next| next.line - 1);

    Some((start.line, end))
}

/// Niveau et texte d'une ligne de titre (`## Titre`), `None` pour toute autre ligne.
fn parse_heading(line: &str) -> Option<(usize, String)> {
    let level = line.chars().take_while(|&c| c == '#').count();
    let text = line[level..].strip_prefix(' ')?;

    (1..=6)
        .contains(&level)
        .then(|| (level, text.trim().to_owned()))
}

/// `old` et `new` aux fins de ligne du texte : une IA envoie `\n`, une note importée peut
/// porter `\r\n`.
fn in_line_endings_of(content: &str, old: &str, new: &str) -> (String, String) {
    if content.contains("\r\n") && !old.contains('\r') {
        (old.replace('\n', "\r\n"), new.replace('\n', "\r\n"))
    } else {
        (old.to_owned(), new.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_a_unique_passage() {
        let edited = replace_passage("one two three", "two", "2", false).unwrap();

        assert_eq!(edited, "one 2 three");
    }

    #[test]
    fn refuses_an_empty_missing_or_ambiguous_passage() {
        assert_eq!(
            replace_passage("a", "", "b", false),
            Err(ReplaceError::EmptyPassage)
        );
        assert_eq!(
            replace_passage("a", "z", "b", false),
            Err(ReplaceError::NotFound)
        );
        assert_eq!(
            replace_passage("a a a", "a", "b", false),
            Err(ReplaceError::Ambiguous(3))
        );
    }

    #[test]
    fn replaces_every_occurrence_when_asked() {
        assert_eq!(replace_passage("a a a", "a", "b", true).unwrap(), "b b b");
    }

    #[test]
    fn deletes_a_passage_replaced_by_nothing() {
        assert_eq!(replace_passage("a-b-c", "-b", "", false).unwrap(), "a-c");
    }

    #[test]
    fn matches_multiline_passages_in_crlf_notes() {
        let edited = replace_passage("a\r\nb\r\nc", "a\nb", "x\ny", false).unwrap();

        assert_eq!(edited, "x\r\ny\r\nc");
    }

    #[test]
    fn slices_lines_within_the_text() {
        let text = "a\nb\nc\n";

        assert_eq!(lines_between(text, 1, 1), "a\n");
        assert_eq!(lines_between(text, 2, 99), "b\nc\n");
        assert_eq!(lines_between(text, 0, 2), "a\nb\n");
        assert_eq!(lines_between(text, 4, 9), "");
        assert_eq!(lines_between(text, 3, 2), "");
        assert_eq!(line_count(text), 3);
        assert_eq!(line_count(""), 0);
    }

    #[test]
    fn lists_headings_outside_code_blocks() {
        let text = "# A\n```\n# not a title\n```\n##B\n## B\n####### seven\n";
        let found: Vec<_> = headings(text)
            .into_iter()
            .map(|h| (h.level, h.text, h.line))
            .collect();

        assert_eq!(found, [(1, "A".to_owned(), 1), (2, "B".to_owned(), 6)]);
    }

    #[test]
    fn a_section_runs_to_the_next_heading_of_its_level_or_higher() {
        let text = "# A\nx\n## B\ny\n### C\nz\n## D\nw\n# E\nv";

        assert_eq!(section_lines(text, "B"), Some((3, 6)));
        assert_eq!(section_lines(text, "A"), Some((1, 8)));
        assert_eq!(section_lines(text, "e"), Some((9, 10)));
        assert_eq!(section_lines(text, "missing"), None);
    }
}
