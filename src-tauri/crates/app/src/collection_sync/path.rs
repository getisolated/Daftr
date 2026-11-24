//! Chemin relatif d'un fichier synchronisé : validé une fois, à l'entrée.
//!
//! Un chemin reçu d'un autre appareil n'est jamais fiable. [`SyncPath`] refuse tout ce qui
//! pourrait sortir du dossier de la collection ou n'a pas de sens sur un autre système
//! (`..`, chemin absolu, `\`, lecteur, noms réservés de Windows, flux de données `:`).

use std::fmt;

use domain::collection::{MAX_SCAN_DEPTH, is_excluded_folder, is_markdown_file};

/// Longueur maximale d'un chemin, en octets.
const MAX_PATH_BYTES: usize = 1024;
/// Longueur maximale d'un nom, en octets (limite des systèmes de fichiers courants).
const MAX_NAME_BYTES: usize = 255;

/// Noms que Windows réserve (avec ou sans extension, quelle que soit la casse).
const RESERVED: [&str; 4] = ["CON", "PRN", "AUX", "NUL"];

/// Pourquoi un chemin est refusé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathRefusal {
    /// Vide, ou un segment vide (`a//b`).
    Empty,
    /// Trop long.
    TooLong,
    /// Absolu, avec lecteur, ou avec `\`.
    Absolute,
    /// `.` ou `..`.
    Traversal,
    /// Caractère interdit (contrôle, `:` `*` `?` `"` `<` `>` `|`).
    ForbiddenCharacter,
    /// Nom réservé de Windows (`CON`, `COM1`, `LPT3`…).
    ReservedName,
    /// Nom finissant par un point ou une espace (que Windows retire).
    TrailingDotOrSpace,
    /// Le fichier n'est pas un `.md`.
    NotMarkdown,
    /// Dans un dossier exclu ou caché, ou trop profond.
    Excluded,
}

/// Chemin relatif valide d'un fichier markdown, séparateurs `/`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SyncPath(String);

impl SyncPath {
    /// Le chemin `text`, `excludes` étant les dossiers ignorés (les dossiers cachés le sont
    /// toujours).
    ///
    /// ```
    /// use app::collection_sync::SyncPath;
    ///
    /// assert!(SyncPath::parse("docs/a.md", &[]).is_ok());
    /// assert!(SyncPath::parse("../a.md", &[]).is_err());
    /// assert!(SyncPath::parse("docs/image.png", &[]).is_err());
    /// ```
    pub fn parse(text: &str, excludes: &[String]) -> Result<Self, PathRefusal> {
        if text.is_empty() {
            return Err(PathRefusal::Empty);
        }
        if text.len() > MAX_PATH_BYTES {
            return Err(PathRefusal::TooLong);
        }
        if text.starts_with('/') || text.contains('\\') {
            return Err(PathRefusal::Absolute);
        }
        let segments: Vec<&str> = text.split('/').collect();
        if segments.len() > MAX_SCAN_DEPTH + 1 {
            return Err(PathRefusal::Excluded);
        }
        let (name, folders) = segments.split_last().ok_or(PathRefusal::Empty)?;

        for folder in folders {
            check_segment(folder)?;
            if is_excluded_folder(folder, excludes) {
                return Err(PathRefusal::Excluded);
            }
        }
        check_segment(name)?;
        if !is_markdown_file(name) {
            return Err(PathRefusal::NotMarkdown);
        }
        Ok(Self(text.to_owned()))
    }

    /// Le chemin, séparateurs `/`.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Le nom du fichier.
    pub fn file_name(&self) -> &str {
        self.0.rsplit('/').next().unwrap_or(&self.0)
    }

    /// Clé de comparaison insensible à la casse : deux chemins de même clé se confondent sur
    /// Windows et macOS.
    pub fn fold_key(&self) -> String {
        self.0.to_lowercase()
    }
}

impl fmt::Display for SyncPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl fmt::Debug for SyncPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "SyncPath({})", self.0)
    }
}

fn check_segment(segment: &str) -> Result<(), PathRefusal> {
    if segment.is_empty() {
        return Err(PathRefusal::Empty);
    }
    if segment == "." || segment == ".." {
        return Err(PathRefusal::Traversal);
    }
    if segment.len() > MAX_NAME_BYTES {
        return Err(PathRefusal::TooLong);
    }
    if segment.chars().any(is_forbidden) {
        return Err(PathRefusal::ForbiddenCharacter);
    }
    if segment.ends_with('.') || segment.ends_with(' ') {
        return Err(PathRefusal::TrailingDotOrSpace);
    }
    if is_reserved(segment) {
        return Err(PathRefusal::ReservedName);
    }
    Ok(())
}

fn is_forbidden(c: char) -> bool {
    c.is_control() || matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|')
}

/// `CON`, `NUL.txt`, `com1`, `LPT9.md` : le nom avant le premier point est réservé.
fn is_reserved(segment: &str) -> bool {
    let stem = segment.split('.').next().unwrap_or(segment).to_uppercase();
    let numbered = |prefix: &str| {
        stem.strip_prefix(prefix)
            .is_some_and(|digit| matches!(digit.as_bytes(), [b'0'..=b'9']))
    };
    RESERVED.contains(&stem.as_str()) || numbered("COM") || numbered("LPT")
}
