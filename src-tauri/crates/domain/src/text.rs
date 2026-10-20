//! Petits utilitaires de texte partagés par les modules du domaine.

/// Arrondi à l'entier le plus proche, les demis vers +∞ (comme `Math.round` côté renderer).
pub(crate) fn round_half_up(value: f64) -> f64 {
    (value + 0.5).floor()
}

/// Les `max` premiers caractères de `text`.
pub(crate) fn truncate_chars(text: &str, max: usize) -> &str {
    match text.char_indices().nth(max) {
        Some((index, _)) => &text[..index],
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_halves_up_like_the_renderer() {
        assert_eq!(round_half_up(10.4), 10.0);
        assert_eq!(round_half_up(20.5), 21.0);
        assert_eq!(round_half_up(-2.5), -2.0);
    }

    #[test]
    fn truncates_on_character_boundaries() {
        assert_eq!(truncate_chars("Été", 2), "Ét");
        assert_eq!(truncate_chars("abc", 10), "abc");
    }
}
