//! Noms saisis par l'utilisateur pour les fichiers, dossiers et collections.

use super::is_markdown_file;
use crate::text::truncate_chars;

/// Longueur maximale du nom affiché d'une collection.
pub const MAX_DISPLAY_NAME_LENGTH: usize = 100;

/// Un nom de fichier ou de dossier valide (sans les espaces autour), `None` s'il est vide,
/// réservé (`.`, `..`) ou contient un caractère interdit par Windows.
pub fn entry_name(name: &str) -> Option<String> {
    let trimmed = name.trim();
    let forbidden = |c: char| {
        matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || c <= '\u{1f}'
    };
    let valid =
        !trimmed.is_empty() && trimmed != "." && trimmed != ".." && !trimmed.contains(forbidden);

    valid.then(|| trimmed.to_owned())
}

/// Le nom avec l'extension `.md` ajoutée s'il ne l'a pas.
pub fn with_markdown_extension(name: &str) -> String {
    if is_markdown_file(name) {
        name.to_owned()
    } else {
        format!("{name}.md")
    }
}

/// Nom affiché d'une collection : le nom demandé (coupé), ou `fallback` s'il est vide.
pub fn display_name(requested: Option<&str>, fallback: &str) -> String {
    let trimmed = requested.map(str::trim).unwrap_or_default();

    if trimmed.is_empty() {
        return fallback.to_owned();
    }
    truncate_chars(trimmed, MAX_DISPLAY_NAME_LENGTH).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_plain_name() {
        assert_eq!(entry_name("  Guide  ").as_deref(), Some("Guide"));
        assert_eq!(entry_name("été.md").as_deref(), Some("été.md"));
    }

    #[test]
    fn refuses_empty_reserved_and_forbidden_names() {
        for name in ["", "  ", ".", "..", "a/b", "a\\b", "a:b", "a?", "a\u{1}b"] {
            assert_eq!(entry_name(name), None, "{name:?}");
        }
    }

    #[test]
    fn adds_the_markdown_extension_once() {
        assert_eq!(with_markdown_extension("Guide"), "Guide.md");
        assert_eq!(with_markdown_extension("Guide.MD"), "Guide.MD");
    }

    #[test]
    fn falls_back_to_the_default_display_name() {
        assert_eq!(display_name(Some(" Docs "), "repo"), "Docs");
        assert_eq!(display_name(Some("  "), "repo"), "repo");
        assert_eq!(display_name(None, "repo"), "repo");
        assert_eq!(
            display_name(Some(&"x".repeat(150)), "repo").len(),
            MAX_DISPLAY_NAME_LENGTH
        );
    }
}
