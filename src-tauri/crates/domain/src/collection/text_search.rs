//! Recherche de texte dans les fichiers des collections.
//!
//! Les positions rendues sont en unités UTF-16, celles des chaînes JavaScript du renderer.

/// Longueur maximale (en caractères) d'une ligne rendue.
const MAX_MATCH_LINE_LENGTH: usize = 160;

/// Caractères gardés avant la correspondance quand la ligne est coupée.
const CONTEXT_BEFORE: usize = 40;

/// Une ligne d'un fichier contenant la recherche.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMatch {
    /// Numéro de ligne, à partir de 1.
    pub line: usize,
    /// La ligne, coupée autour de la correspondance.
    pub text: String,
    /// Début de la correspondance dans `text` (unités UTF-16).
    pub from: usize,
    /// Fin de la correspondance dans `text` (unités UTF-16).
    pub to: usize,
}

/// Les lignes de `content` contenant `query` (sans tenir compte de la casse), au plus `limit`.
///
/// ```
/// let matches = domain::collection::find_in_text("Alpha\nthe BETA line", "beta", 5);
/// assert_eq!((matches[0].line, matches[0].from, matches[0].to), (2, 4, 8));
/// ```
pub fn find_in_text(content: &str, query: &str, limit: usize) -> Vec<FileMatch> {
    let needle = lowercase_chars(query);
    if needle.is_empty() {
        return Vec::new();
    }
    content
        .lines()
        .enumerate()
        .filter_map(|(index, line)| match_line(index + 1, line, &needle))
        .take(limit)
        .collect()
}

/// `name` contient `query`, sans tenir compte de la casse.
pub fn contains_ignore_case(name: &str, query: &str) -> bool {
    name.to_lowercase().contains(&query.to_lowercase())
}

fn match_line(number: usize, line: &str, needle: &[char]) -> Option<FileMatch> {
    let chars: Vec<char> = line.chars().collect();
    let column = find_chars(&lowercase_chars(line), needle)?;
    let start = column.saturating_sub(CONTEXT_BEFORE);
    let end = (start + MAX_MATCH_LINE_LENGTH).min(chars.len());
    let kept = &chars[start..end];
    let match_end = (column - start + needle.len()).min(kept.len());

    Some(FileMatch {
        line: number,
        text: kept.iter().collect(),
        from: utf16_length(&kept[..column - start]),
        to: utf16_length(&kept[..match_end]),
    })
}

/// Minuscules caractère par caractère, sans changer le nombre de caractères : les positions
/// trouvées restent valables dans le texte d'origine.
fn lowercase_chars(text: &str) -> Vec<char> {
    text.chars()
        .map(|c| c.to_lowercase().next().unwrap_or(c))
        .collect()
}

fn find_chars(haystack: &[char], needle: &[char]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn utf16_length(chars: &[char]) -> usize {
    chars.iter().map(|c| c.len_utf16()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(line: usize, text: &str, from: usize, to: usize) -> FileMatch {
        FileMatch {
            line,
            text: text.to_owned(),
            from,
            to,
        }
    }

    #[test]
    fn finds_matching_lines_case_insensitively() {
        assert_eq!(
            find_in_text("Alpha\nthe BETA line\r\nbeta again", "beta", 5),
            [
                found(2, "the BETA line", 4, 8),
                found(3, "beta again", 0, 4),
            ]
        );
    }

    #[test]
    fn stops_at_the_limit_and_ignores_an_empty_query() {
        assert_eq!(find_in_text("a\na\na", "a", 2).len(), 2);
        assert!(find_in_text("a", "", 5).is_empty());
    }

    #[test]
    fn cuts_long_lines_around_the_match() {
        let line = format!("{}needle{}", "x".repeat(100), "y".repeat(300));
        let matches = find_in_text(&line, "needle", 1);
        let first = &matches[0];

        assert_eq!(&first.text[first.from..first.to], "needle");
        assert!(first.text.chars().count() <= MAX_MATCH_LINE_LENGTH);
    }

    #[test]
    fn counts_positions_in_utf16_units() {
        let matches = find_in_text("😀 Été", "été", 1);
        assert_eq!((matches[0].from, matches[0].to), (3, 6));
    }

    #[test]
    fn matches_file_names_case_insensitively() {
        assert!(contains_ignore_case("docs/Guide.md", "guide"));
        assert!(!contains_ignore_case("docs/Guide.md", "api"));
    }
}
