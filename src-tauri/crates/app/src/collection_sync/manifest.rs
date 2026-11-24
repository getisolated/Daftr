//! Manifeste : ce qu'un appareil annonce de sa copie d'une collection.

use std::collections::{HashMap, HashSet};

use super::{FileHash, SyncPath};

/// Pourquoi un fichier n'est pas synchronisé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IgnoredReason {
    /// Plus gros que [`super::MAX_FILE_BYTES`].
    TooLarge,
    /// Un autre fichier ne diffère de lui que par la casse du chemin.
    CaseCollision,
    /// Nom que les autres systèmes ne peuvent pas porter.
    InvalidPath,
    /// Illisible (droits, lien symbolique, fichier disparu pendant la lecture).
    Unreadable,
}

/// Un fichier écarté, pour le message affiché à l'utilisateur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ignored {
    /// Chemin relatif tel que trouvé.
    pub path: String,
    /// La raison.
    pub reason: IgnoredReason,
}

/// Un fichier présent sur le disque, hachage compris.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFile {
    /// Chemin relatif.
    pub path: SyncPath,
    /// Empreinte du contenu.
    pub hash: FileHash,
    /// Taille en octets.
    pub size: u64,
}

/// Une ligne du manifeste.
///
/// `hash` absent : le fichier n'existe pas (ou plus) ici, mais la dernière version échangée
/// avec cet appareil (`base`) est connue : c'est une suppression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    /// Chemin relatif.
    pub path: SyncPath,
    /// Empreinte actuelle, absente pour une suppression.
    pub hash: Option<FileHash>,
    /// Taille actuelle (0 pour une suppression).
    pub size: u64,
    /// Dernière version échangée avec l'appareil qui reçoit ce manifeste.
    pub base: Option<FileHash>,
}

/// Le manifeste d'une copie locale, trié par chemin.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    /// Les lignes.
    pub entries: Vec<FileEntry>,
    /// Les fichiers écartés (casse en collision).
    pub ignored: Vec<Ignored>,
}

impl Manifest {
    /// La ligne d'un chemin.
    pub fn get(&self, path: &SyncPath) -> Option<&FileEntry> {
        self.entries
            .binary_search_by(|entry| entry.path.cmp(path))
            .ok()
            .and_then(|index| self.entries.get(index))
    }
}

/// Le manifeste de `files` pour un appareil dont les dernières versions échangées sont `bases`.
///
/// Deux fichiers qui ne diffèrent que par la casse sont écartés tous les deux : sur un système
/// insensible à la casse, l'un écraserait l'autre.
pub fn build_manifest(files: Vec<LocalFile>, bases: &HashMap<SyncPath, FileHash>) -> Manifest {
    let colliding = colliding_keys(files.iter().map(|file| &file.path));
    let mut manifest = Manifest::default();
    let mut present: HashSet<SyncPath> = HashSet::new();

    for file in files {
        if colliding.contains(&file.path.fold_key()) {
            manifest.ignored.push(ignored_case(&file.path));
            continue;
        }
        manifest.entries.push(FileEntry {
            base: bases.get(&file.path).copied(),
            hash: Some(file.hash),
            size: file.size,
            path: file.path.clone(),
        });
        present.insert(file.path);
    }
    for (path, base) in bases {
        if !present.contains(path) && !colliding.contains(&path.fold_key()) {
            manifest.entries.push(FileEntry {
                path: path.clone(),
                hash: None,
                size: 0,
                base: Some(*base),
            });
        }
    }
    manifest.entries.sort_by(|a, b| a.path.cmp(&b.path));
    manifest.ignored.sort_by(|a, b| a.path.cmp(&b.path));
    manifest
}

/// Les clés insensibles à la casse portées par au moins deux chemins différents.
pub(super) fn colliding_keys<'a>(paths: impl Iterator<Item = &'a SyncPath>) -> HashSet<String> {
    let mut seen: HashMap<String, &SyncPath> = HashMap::new();
    let mut colliding = HashSet::new();

    for path in paths {
        match seen.insert(path.fold_key(), path) {
            Some(previous) if previous != path => {
                colliding.insert(path.fold_key());
            }
            _ => {}
        }
    }
    colliding
}

pub(super) fn ignored_case(path: &SyncPath) -> Ignored {
    Ignored {
        path: path.as_str().to_owned(),
        reason: IgnoredReason::CaseCollision,
    }
}
