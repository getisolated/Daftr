//! Images référencées par les fichiers markdown d'une collection.

/// Type MIME d'une image d'après son extension (sans tenir compte de la casse), `None` pour
/// tout autre fichier : seules les images d'une collection sont servies à l'aperçu.
///
/// ```
/// assert_eq!(domain::collection::image_mime("Plan.GIF"), Some("image/gif"));
/// assert_eq!(domain::collection::image_mime("notes.md"), None);
/// ```
pub fn image_mime(name: &str) -> Option<&'static str> {
    let (_, extension) = name.rsplit_once('.')?;

    match extension.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "svg" => Some("image/svg+xml"),
        "bmp" => Some("image/bmp"),
        "ico" => Some("image/x-icon"),
        "avif" => Some("image/avif"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_common_image_formats() {
        assert_eq!(image_mime("a.png"), Some("image/png"));
        assert_eq!(image_mime("a.JPEG"), Some("image/jpeg"));
        assert_eq!(image_mime("schema.svg"), Some("image/svg+xml"));
    }

    #[test]
    fn refuses_everything_else() {
        for name in ["notes.md", "script.ps1", "secret", "archive.zip", ".png"] {
            assert_eq!(image_mime(name).is_some(), name == ".png", "{name}");
        }
    }
}
