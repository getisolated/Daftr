//! Icône d'un profil : une petite image carrée, recadrée et réduite par le renderer, vérifiée
//! ici avant d'être écrite sur le disque.

/// Taille maximale d'une icône, en octets (une image de 256 px en PNG tient largement).
pub const MAX_ICON_BYTES: usize = 512 * 1024;

const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";

text_enum! {
    /// Format d'une icône de profil.
    pub enum IconFormat {
        /// PNG.
        Png => "png",
        /// WebP.
        Webp => "webp",
    }
}

impl IconFormat {
    /// Nom du fichier de l'icône dans le dossier du profil : fixé par le format, jamais tiré
    /// d'une entrée.
    pub const fn file_name(self) -> &'static str {
        match self {
            Self::Png => "profile-icon.png",
            Self::Webp => "profile-icon.webp",
        }
    }

    /// Type MIME.
    pub const fn mime(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Webp => "image/webp",
        }
    }
}

/// Le format de l'image si elle est acceptable comme icône (PNG ou WebP reconnu à sa
/// signature, pas vide ni trop lourde), `None` sinon.
///
/// ```
/// use domain::profile::{IconFormat, validate_icon};
/// assert_eq!(validate_icon(b"\x89PNG\r\n\x1a\n...."), Some(IconFormat::Png));
/// assert_eq!(validate_icon(b"GIF89a"), None);
/// ```
pub fn validate_icon(bytes: &[u8]) -> Option<IconFormat> {
    if bytes.len() > MAX_ICON_BYTES {
        return None;
    }
    if bytes.starts_with(PNG_SIGNATURE) && bytes.len() > PNG_SIGNATURE.len() {
        return Some(IconFormat::Png);
    }
    is_webp(bytes).then_some(IconFormat::Webp)
}

/// Conteneur RIFF de type `WEBP`, suivi d'au moins un bloc.
fn is_webp(bytes: &[u8]) -> bool {
    bytes.len() > 12 && bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(size: usize) -> Vec<u8> {
        let mut bytes = PNG_SIGNATURE.to_vec();
        bytes.resize(size, 0);
        bytes
    }

    #[test]
    fn recognizes_png_and_webp() {
        assert_eq!(validate_icon(&png(64)), Some(IconFormat::Png));
        assert_eq!(
            validate_icon(b"RIFF\x20\0\0\0WEBPVP8 data"),
            Some(IconFormat::Webp)
        );
    }

    #[test]
    fn refuses_other_or_truncated_images() {
        assert_eq!(validate_icon(b""), None);
        assert_eq!(validate_icon(PNG_SIGNATURE), None);
        assert_eq!(validate_icon(b"RIFF\0\0\0\0WEBP"), None);
        assert_eq!(validate_icon(b"RIFF\0\0\0\0WAVEfmt data"), None);
        assert_eq!(validate_icon(b"\xff\xd8\xff\xe0 jpeg"), None);
        assert_eq!(validate_icon(b"<svg onload=alert(1)>"), None);
    }

    #[test]
    fn refuses_an_image_over_the_limit() {
        assert!(validate_icon(&png(MAX_ICON_BYTES)).is_some());
        assert_eq!(validate_icon(&png(MAX_ICON_BYTES + 1)), None);
    }

    #[test]
    fn names_the_file_after_the_format_only() {
        assert_eq!(IconFormat::Png.file_name(), "profile-icon.png");
        assert_eq!(IconFormat::Webp.mime(), "image/webp");
        assert_eq!(IconFormat::parse("webp"), Some(IconFormat::Webp));
        assert_eq!(IconFormat::parse("../x"), None);
    }
}
