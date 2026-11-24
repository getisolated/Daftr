//! Décision par fichier : recevoir, supprimer, fusionner ou ne rien faire.
//!
//! Trois empreintes par chemin : la locale `L`, la distante `R` et la base `B` (dernière
//! version échangée, que les deux côtés doivent connaître à l'identique, sinon elle est
//! ignorée). Les règles :
//!
//! | `L` | `R` | `B` | décision |
//! |---|---|---|---|
//! | `= R` | | | rien (la base est mise à jour) |
//! | présent | absent | aucune | l'autre reçoit |
//! | absent | présent | aucune | je reçois |
//! | présent | présent | aucune | conflit |
//! | `= B` | absent | `B` | je supprime (corbeille) |
//! | `= B` | présent | `B` | je reçois |
//! | `≠ B` | `= B` | `B` | l'autre reçoit ou supprime |
//! | présent | absent | `B`, `L ≠ B` | l'autre reçoit : une modification gagne sur une suppression |
//! | absent | présent | `B`, `R ≠ B` | je reçois : idem |
//! | présent | présent | `B`, tous deux `≠ B` | conflit |

use std::collections::{BTreeSet, HashMap};

use super::manifest::colliding_keys;
use super::{FileEntry, FileHash, IgnoredReason, MAX_FILE_BYTES, Manifest, SyncPath};

/// Ce que cet appareil fait d'un fichier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileAction {
    /// Demander la version distante et l'écrire.
    Pull,
    /// Envoyer le fichier local à la corbeille : l'autre appareil l'a supprimé.
    DeleteLocal,
    /// Conflit : demander la version distante, puis fusionner ou garder les deux.
    Reconcile,
    /// Même contenu des deux côtés : le retenir comme dernière version échangée.
    Adopt,
    /// Absent des deux côtés : oublier la dernière version échangée.
    Forget,
    /// Fichier non synchronisé.
    Skip(IgnoredReason),
}

/// Une décision, avec la version distante visée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFile {
    /// Le fichier.
    pub path: SyncPath,
    /// Ce qu'il faut en faire.
    pub action: FileAction,
    /// Empreinte locale au moment du plan : l'écriture n'a lieu que si elle n'a pas changé.
    pub local: Option<FileHash>,
    /// Empreinte distante (absente pour une suppression).
    pub remote: Option<FileHash>,
    /// Base commune des deux côtés, si elle existe (celle d'une fusion).
    pub base: Option<FileHash>,
    /// Taille distante annoncée.
    pub size: u64,
}

/// Le plan de cet appareil : une ligne par fichier à traiter (les fichiers déjà à jour et
/// ceux que l'autre appareil doit recevoir n'y figurent pas).
pub fn plan(local: &Manifest, remote: &[FileEntry]) -> Vec<PlannedFile> {
    let remote_by_path: HashMap<&SyncPath, &FileEntry> =
        remote.iter().map(|entry| (&entry.path, entry)).collect();
    let remote_collisions = colliding_keys(remote.iter().map(|entry| &entry.path));
    let local_keys = local_keys(local);
    let paths: BTreeSet<&SyncPath> = local
        .entries
        .iter()
        .map(|entry| &entry.path)
        .chain(remote.iter().map(|entry| &entry.path))
        .collect();
    let mut planned = Vec::new();

    for path in paths {
        let mine = local.get(path);
        let theirs = remote_by_path.get(path).copied();
        let skip = skip_reason(path, theirs, &remote_collisions, &local_keys);
        let action = match skip {
            Some(reason) => Some(FileAction::Skip(reason)),
            None => action_for(mine, theirs),
        };
        if let Some(action) = action {
            planned.push(PlannedFile {
                path: path.clone(),
                action,
                local: mine.and_then(|entry| entry.hash),
                remote: theirs.and_then(|entry| entry.hash),
                base: effective_base(mine, theirs),
                size: theirs.map_or(0, |entry| entry.size),
            });
        }
    }
    planned
}

/// Les chemins locaux (présents ou écartés pour leur casse), par clé insensible à la casse.
fn local_keys(local: &Manifest) -> HashMap<String, SyncPath> {
    let present = local
        .entries
        .iter()
        .filter(|entry| entry.hash.is_some())
        .map(|entry| entry.path.clone());
    let ignored = local
        .ignored
        .iter()
        .filter_map(|ignored| SyncPath::parse(&ignored.path, &[]).ok());

    present
        .chain(ignored)
        .map(|path| (path.fold_key(), path))
        .collect()
}

/// Un fichier distant qu'on ne peut pas recevoir : casse en collision, trop gros.
fn skip_reason(
    path: &SyncPath,
    theirs: Option<&FileEntry>,
    remote_collisions: &std::collections::HashSet<String>,
    local_keys: &HashMap<String, SyncPath>,
) -> Option<IgnoredReason> {
    let theirs = theirs?;
    theirs.hash?;
    let key = path.fold_key();
    let clashes_here = local_keys.get(&key).is_some_and(|other| other != path);

    if remote_collisions.contains(&key) || clashes_here {
        return Some(IgnoredReason::CaseCollision);
    }
    (theirs.size > MAX_FILE_BYTES).then_some(IgnoredReason::TooLarge)
}

fn action_for(mine: Option<&FileEntry>, theirs: Option<&FileEntry>) -> Option<FileAction> {
    let local = mine.and_then(|entry| entry.hash);
    let remote = theirs.and_then(|entry| entry.hash);
    let own_base = mine.and_then(|entry| entry.base);
    let base = effective_base(mine, theirs);

    if local == remote {
        return match local {
            Some(hash) => (own_base != Some(hash)).then_some(FileAction::Adopt),
            None => own_base.map(|_| FileAction::Forget),
        };
    }
    decide(local, remote, base)
}

/// La base que les deux côtés déduisent à l'identique : celle qu'ils annoncent tous deux, ou,
/// quand un accusé s'est perdu, celle que l'un annonce et que l'autre a exactement dans son
/// fichier (la version que l'un croit avoir échangée est bien celle de l'autre).
fn effective_base(mine: Option<&FileEntry>, theirs: Option<&FileEntry>) -> Option<FileHash> {
    let local = mine.and_then(|entry| entry.hash);
    let remote = theirs.and_then(|entry| entry.hash);
    let own = mine.and_then(|entry| entry.base);
    let other = theirs.and_then(|entry| entry.base);

    if own.is_some() && own == other {
        return own;
    }
    if other.is_some() && other == local {
        return other;
    }
    own.filter(|base| Some(*base) == remote)
}

/// `L ≠ R`, la table du module.
fn decide(
    local: Option<FileHash>,
    remote: Option<FileHash>,
    base: Option<FileHash>,
) -> Option<FileAction> {
    match (local, remote) {
        (Some(_), None) if local == base => Some(FileAction::DeleteLocal),
        (Some(_), None) => None,
        (None, Some(_)) if remote == base => None,
        (None, Some(_)) => Some(FileAction::Pull),
        (Some(_), Some(_)) if local == base => Some(FileAction::Pull),
        (Some(_), Some(_)) if remote == base => None,
        (Some(_), Some(_)) => Some(FileAction::Reconcile),
        (None, None) => None,
    }
}
