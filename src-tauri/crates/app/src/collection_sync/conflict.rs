//! Conflits : fusion à trois voies d'un texte, sinon copie nommée `nom (conflit <appareil>).md`.
//!
//! Les deux côtés d'un conflit calculent le même résultat à partir des mêmes entrées : la
//! fusion donne le même texte des deux côtés (l'ordre des insertions est celui de
//! l'arbitre d'abord, pas celui de « local » puis « distant »), et seul le suiveur crée la
//! copie, de sorte que rien n'est écrit deux fois.

use super::{MAX_FILE_BYTES, SyncPath};
use crate::sync::text_merge::merge;

/// Nombre maximal de caractères du nom d'appareil dans un nom de copie.
const MAX_LABEL_CHARS: usize = 40;
/// Nom de repli quand le nom d'appareil ne donne rien d'utilisable dans un nom de fichier.
const FALLBACK_LABEL: &str = "appareil";
const MARKER: &str = " (conflit ";

/// Rôle de cet appareil dans un conflit : le plus petit identifiant arbitre, ce qui donne aux
/// deux côtés la même issue sans échange supplémentaire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Garde sa version à la place d'origine.
    Arbiter,
    /// Met sa version de côté (copie de conflit) et prend celle de l'arbitre.
    Follower,
}

impl Role {
    /// Le rôle de l'appareil `local` face à l'appareil `remote`.
    pub fn between<K: Ord>(local: &K, remote: &K) -> Self {
        if local < remote {
            Self::Arbiter
        } else {
            Self::Follower
        }
    }
}

/// L'issue d'un conflit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// Texte fusionné, à écrire des deux côtés.
    Merged(Vec<u8>),
    /// Pas de fusion possible : l'arbitre garde sa version, le suiveur met la sienne dans une
    /// copie de conflit et prend celle de l'arbitre.
    KeepBoth,
}

/// Résout un conflit. La fusion exige une base connue et trois textes UTF-8 ; sinon les deux
/// versions sont gardées.
pub fn resolve(role: Role, base: Option<&[u8]>, local: &[u8], remote: &[u8]) -> Resolution {
    let texts = (
        base.and_then(|bytes| std::str::from_utf8(bytes).ok()),
        std::str::from_utf8(local).ok(),
        std::str::from_utf8(remote).ok(),
    );
    let (Some(base), Some(local), Some(remote)) = texts else {
        return Resolution::KeepBoth;
    };
    let (arbiter, follower) = match role {
        Role::Arbiter => (local, remote),
        Role::Follower => (remote, local),
    };
    let merged = merge(base, arbiter, follower);

    if u64::try_from(merged.len()).is_ok_and(|len| len <= MAX_FILE_BYTES) {
        Resolution::Merged(merged.into_bytes())
    } else {
        Resolution::KeepBoth
    }
}

/// Une copie de conflit reconnue d'après son nom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConflictName {
    /// Chemin du fichier d'origine.
    pub original: String,
    /// Nom d'appareil écrit dans le nom de la copie (avec son numéro éventuel).
    pub label: String,
}

impl ConflictName {
    /// La copie porte la version de l'appareil nommé `device`.
    pub fn is_from(&self, device: &str) -> bool {
        let label = sanitize_label(device);
        let unnumbered = self
            .label
            .rsplit_once(' ')
            .filter(|(_, number)| number.chars().all(|c| c.is_ascii_digit()))
            .map(|(name, _)| name);

        self.label == label || unnumbered == Some(label.as_str())
    }
}

/// Le nom de la copie de conflit de `path` pour la version de l'appareil `device` :
/// `dir/nom (conflit appareil).md`, numérotée (`appareil 2`) tant que le nom est pris.
pub fn conflict_name(path: &SyncPath, device: &str, taken: impl Fn(&str) -> bool) -> SyncPath {
    let (folder, stem, extension) = split(path.as_str());
    let label = sanitize_label(device);

    for number in 1_u32.. {
        let suffix = if number == 1 {
            label.clone()
        } else {
            format!("{label} {number}")
        };
        let candidate = format!("{folder}{stem}{MARKER}{suffix}){extension}");

        if let Ok(valid) = SyncPath::parse(&candidate, &[])
            && !taken(valid.as_str())
        {
            return valid;
        }
        if number > 1000 {
            break;
        }
    }
    path.clone()
}

/// `path` est une copie de conflit : `dir/nom (conflit appareil).md`.
pub fn parse_conflict(path: &str) -> Option<ConflictName> {
    let (folder, stem, extension) = split(path);
    let body = stem.strip_suffix(')')?;
    let (original, label) = body.rsplit_once(MARKER)?;

    (!original.is_empty() && !label.is_empty()).then(|| ConflictName {
        original: format!("{folder}{original}{extension}"),
        label: label.to_owned(),
    })
}

/// Dossier (avec son `/` final), nom sans extension, extension (`.md`, casse d'origine).
fn split(path: &str) -> (&str, &str, &str) {
    let name_start = path.rfind('/').map_or(0, |index| index + 1);
    let (folder, name) = path.split_at(name_start);
    let stem_end = name.len().saturating_sub(3);
    let (stem, extension) = if name.is_char_boundary(stem_end) {
        name.split_at(stem_end)
    } else {
        (name, "")
    };

    (folder, stem, extension)
}

/// Un nom d'appareil utilisable dans un nom de fichier.
fn sanitize_label(device: &str) -> String {
    let cleaned: String = device
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '(' | ')' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .take(MAX_LABEL_CHARS)
        .collect();
    let trimmed = cleaned.trim_matches(|c: char| c == ' ' || c == '.');

    if trimmed.is_empty() {
        FALLBACK_LABEL.to_owned()
    } else {
        trimmed.to_owned()
    }
}
