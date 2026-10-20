//! Mise en forme des recherches de l'utilisateur pour SQLite (FTS5 et `LIKE`).

/// Chaque mot entre guillemets (guillemets internes doublés) : les opérateurs FTS5
/// (`NOT`, `*`, `-`…) tapés par l'utilisateur deviennent du texte littéral.
///
/// ```
/// assert_eq!(domain::search::sanitize_fts_query("  foo   bar "), r#""foo" "bar""#);
/// ```
pub fn sanitize_fts_query(query: &str) -> String {
    query
        .split_whitespace()
        .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Requête FTS5 de recherche par préfixe du dernier mot, `None` quand il n'y a rien à
/// chercher (la liste complète des notes est alors attendue).
pub fn fts_prefix_query(query: &str) -> Option<String> {
    let sanitized = sanitize_fts_query(query);

    (!sanitized.is_empty()).then(|| format!("{sanitized}*"))
}

/// Échappe les jokers de `LIKE` (`%`, `_` et `\`, le caractère d'échappement choisi).
pub fn escape_like(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Motif `LIKE` (échappement `\`) qui trouve un tag dans la colonne JSON des tags.
pub fn tag_like_pattern(tag: &str) -> String {
    format!("%\"{}\"%", escape_like(&tag.replace('"', "")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_each_word_in_quotes() {
        assert_eq!(sanitize_fts_query("hello"), "\"hello\"");
        assert_eq!(sanitize_fts_query("hello world"), "\"hello\" \"world\"");
    }

    #[test]
    fn escapes_quotes_inside_words() {
        assert_eq!(sanitize_fts_query("say \"hi\""), "\"say\" \"\"\"hi\"\"\"");
    }

    #[test]
    fn trims_and_collapses_spaces() {
        assert_eq!(sanitize_fts_query("  foo   bar  "), "\"foo\" \"bar\"");
        assert_eq!(sanitize_fts_query(""), "");
        assert_eq!(sanitize_fts_query("   "), "");
    }

    #[test]
    fn makes_fts_operators_literal() {
        assert_eq!(sanitize_fts_query("c++"), "\"c++\"");
        assert_eq!(sanitize_fts_query("NOT OR AND"), "\"NOT\" \"OR\" \"AND\"");
    }

    #[test]
    fn builds_a_prefix_query_only_when_there_is_something_to_search() {
        assert_eq!(fts_prefix_query("not"), Some("\"not\"*".to_owned()));
        assert_eq!(fts_prefix_query("  "), None);
    }

    #[test]
    fn escapes_like_wildcards() {
        assert_eq!(escape_like("hello"), "hello");
        assert_eq!(escape_like("100%"), "100\\%");
        assert_eq!(escape_like("file_name"), "file\\_name");
        assert_eq!(escape_like("path\\to"), "path\\\\to");
        assert_eq!(escape_like("%_\\"), "\\%\\_\\\\");
        assert_eq!(escape_like(""), "");
    }

    #[test]
    fn matches_a_whole_tag_in_the_json_column() {
        assert_eq!(tag_like_pattern("to_do"), "%\"to\\_do\"%");
        assert_eq!(tag_like_pattern("a\"b"), "%\"ab\"%");
    }
}
