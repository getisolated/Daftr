//! Fusion d'un profil existant (pas encore synchronisé) dans un espace reçu : décisions
//! pures d'adoption d'uuid, appliquées ensuite par l'adaptateur en une transaction.
//!
//! Un dossier local adopte l'uuid du dossier distant de même chemin (noms depuis la racine),
//! le dossier par défaut adopte l'uuid dérivé de l'espace, et une note locale identique à une
//! note distante du même chemin de dossier adopte son uuid (doublon). Tout le reste est gardé.

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

use domain::sync::{SpaceId, default_folder_uuid};
use domain::uuid::Uuid;

use crate::error::{SyncError, SyncResult};
use crate::sync::note_doc::ProjectedNote;
use crate::sync::space_doc::{self, FolderFields, ProjectedSpace, SpaceDoc};

/// Un dossier du profil local.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFolder {
    /// Uuid local (attribué par la migration).
    pub uuid: Uuid,
    /// Dossier par défaut du profil.
    pub is_default: bool,
    /// Champs ; `parent` désigne un uuid local.
    pub fields: FolderFields,
}

/// Une note du profil local, réduite à ce qui sert au dédoublonnage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalNote {
    /// Uuid local (attribué par la migration).
    pub uuid: Uuid,
    /// Texte markdown complet.
    pub content: String,
    /// Dossier local, `None` à la racine.
    pub folder: Option<Uuid>,
}

/// Ce que la fusion va faire du profil local.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeInput<'a> {
    /// Le profil appartient déjà à un espace : la fusion est refusée.
    pub already_synced: bool,
    /// L'espace rejoint.
    pub space_id: &'a SpaceId,
    /// Dossiers locaux.
    pub local_folders: &'a [LocalFolder],
    /// Notes locales visibles.
    pub local_notes: &'a [LocalNote],
    /// L'espace distant, reçu en premier.
    pub remote_space: &'a ProjectedSpace,
    /// Les notes distantes reçues ; les supprimées sont ignorées.
    pub remote_notes: &'a [ProjectedNote],
}

/// Décisions de la fusion : uuid locaux → uuid adoptés. Ce qui n'y figure pas garde son uuid.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MergePlan {
    /// Dossiers locaux unifiés avec un dossier distant (ou le dossier par défaut dérivé).
    pub folders: BTreeMap<Uuid, Uuid>,
    /// Notes locales identiques à une note distante : la locale prend le document distant.
    pub notes: BTreeMap<Uuid, Uuid>,
}

impl MergePlan {
    /// Uuid final d'un dossier local.
    pub fn folder(&self, local: Uuid) -> Uuid {
        self.folders.get(&local).copied().unwrap_or(local)
    }
}

/// Planifie la fusion. Refusée ([`SyncError::AlreadySynced`]) pour un profil qui appartient
/// déjà à un espace : un profil n'appartient qu'à un seul espace.
pub fn plan(input: &MergeInput<'_>) -> SyncResult<MergePlan> {
    if input.already_synced {
        return Err(SyncError::AlreadySynced);
    }
    let folders = adopt_folders(input);
    let notes = deduplicate_notes(input, &folders);

    Ok(MergePlan { folders, notes })
}

/// Ajoute à l'espace les dossiers locaux qui n'ont pas été unifiés, parents traduits en uuid
/// adoptés. Les dossiers unifiés prennent les valeurs distantes. Rend `true` si le document a
/// changé.
pub fn publish_local_folders(space: &SpaceDoc, plan: &MergePlan, local: &[LocalFolder]) -> bool {
    local
        .iter()
        .filter(|folder| !plan.folders.contains_key(&folder.uuid))
        .fold(false, |changed, folder| {
            let fields = FolderFields {
                parent: folder.fields.parent.map(|parent| plan.folder(parent)),
                ..folder.fields.clone()
            };
            space_doc::put_folder(space, folder.uuid, &fields) | changed
        })
}

fn adopt_folders(input: &MergeInput<'_>) -> BTreeMap<Uuid, Uuid> {
    let local_paths = local_paths(input.local_folders);
    let remote = remote_folders_by_path(input.remote_space);
    let mut taken: BTreeSet<Uuid> = BTreeSet::new();
    let mut adopted = BTreeMap::new();
    let mut ordered: Vec<&LocalFolder> = input.local_folders.iter().collect();
    ordered.sort_by_key(|folder| folder.uuid);

    for folder in ordered {
        let target = if folder.is_default {
            Some(default_folder_uuid(input.space_id))
        } else {
            local_paths
                .get(&folder.uuid)
                .and_then(|path| remote.get(path))
                .copied()
        };
        // Deux dossiers locaux de même chemin : seul le premier est unifié
        if let Some(target) = target.filter(|target| taken.insert(*target)) {
            adopted.insert(folder.uuid, target);
        }
    }
    adopted
}

/// Chemin → plus petit uuid des dossiers distants vivants (hors dossier par défaut).
fn remote_folders_by_path(space: &ProjectedSpace) -> BTreeMap<Vec<&str>, Uuid> {
    let mut by_path = BTreeMap::new();
    for folder in space.folders.iter().filter(|folder| !folder.is_default) {
        if let Some(path) = space.path(folder.uuid) {
            by_path.entry(path).or_insert(folder.uuid);
        }
    }
    by_path
}

/// Chemin de chaque dossier local. Une remontée qui boucle s'arrête au dossier revisité.
fn local_paths(folders: &[LocalFolder]) -> BTreeMap<Uuid, Vec<&str>> {
    let by_uuid: BTreeMap<Uuid, &LocalFolder> =
        folders.iter().map(|folder| (folder.uuid, folder)).collect();

    by_uuid
        .keys()
        .map(|&uuid| (uuid, local_path(&by_uuid, uuid)))
        .collect()
}

fn local_path<'a>(by_uuid: &BTreeMap<Uuid, &'a LocalFolder>, uuid: Uuid) -> Vec<&'a str> {
    let mut path = Vec::new();
    let mut seen = BTreeSet::new();
    let mut current = by_uuid.get(&uuid);

    while let Some(folder) = current.filter(|folder| seen.insert(folder.uuid)) {
        path.push(folder.fields.name.as_str());
        current = folder.fields.parent.and_then(|parent| by_uuid.get(&parent));
    }
    path.reverse();
    path
}

/// Une note locale et une note distante sont des doublons si elles ont le même contenu et le
/// même dossier final (dossier unifié, dossier par défaut ou racine).
fn deduplicate_notes(
    input: &MergeInput<'_>,
    folders: &BTreeMap<Uuid, Uuid>,
) -> BTreeMap<Uuid, Uuid> {
    let mut remote = remote_notes_by_place(input);
    let local_folders: BTreeSet<Uuid> = input
        .local_folders
        .iter()
        .map(|folder| folder.uuid)
        .collect();
    let mut ordered: Vec<&LocalNote> = input.local_notes.iter().collect();
    ordered.sort_by_key(|note| note.uuid);

    let mut duplicates = BTreeMap::new();
    for note in ordered {
        let folder = note.folder.filter(|folder| local_folders.contains(folder));
        let place = (
            folder.map(|folder| folders.get(&folder).copied().unwrap_or(folder)),
            note.content.as_str(),
        );

        if let Some(remote_uuid) = remote.get_mut(&place).and_then(BTreeSet::pop_first) {
            duplicates.insert(note.uuid, remote_uuid);
        }
    }
    duplicates.retain(|local, remote| local != remote);
    duplicates
}

/// Notes distantes visibles, par (dossier effectif, contenu).
fn remote_notes_by_place<'a>(
    input: &MergeInput<'a>,
) -> BTreeMap<(Option<Uuid>, &'a str), BTreeSet<Uuid>> {
    let mut by_place: BTreeMap<(Option<Uuid>, &str), BTreeSet<Uuid>> = BTreeMap::new();

    for note in input
        .remote_notes
        .iter()
        .filter(|note| note.deleted_at.is_none())
    {
        let folder = input.remote_space.resolve_folder(note.folder_uuid);
        by_place
            .entry((folder, note.content.as_str()))
            .or_default()
            .insert(note.uuid);
    }
    by_place
}
