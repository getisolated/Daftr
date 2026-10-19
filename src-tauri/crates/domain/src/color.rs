//! Couleur choisie par l'utilisateur pour une note ou un dossier de la sidebar.

/// Une couleur `#rrggbb` en minuscules, `None` pour toute autre valeur (pas de couleur).
///
/// ```
/// assert_eq!(domain::color::normalize_color(Some("#AABBCC")), Some("#aabbcc".to_owned()));
/// assert_eq!(domain::color::normalize_color(Some("#abc")), None);
/// ```
pub fn normalize_color(value: Option<&str>) -> Option<String> {
    let value = value?;
    let digits = value.strip_prefix('#')?;
    let is_hex = digits.len() == 6 && digits.chars().all(|c| c.is_ascii_hexdigit());

    is_hex.then(|| value.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_a_long_hex_color_in_lower_case() {
        assert_eq!(normalize_color(Some("#AABBCC")).as_deref(), Some("#aabbcc"));
        assert_eq!(normalize_color(Some("#12ab9f")).as_deref(), Some("#12ab9f"));
    }

    #[test]
    fn refuses_anything_else() {
        for value in [
            "#abc",
            "red; drop table",
            "",
            "#12345g",
            "#1234567",
            "aabbcc",
        ] {
            assert_eq!(normalize_color(Some(value)), None, "{value}");
        }
        assert_eq!(normalize_color(None), None);
    }
}
