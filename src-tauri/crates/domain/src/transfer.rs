//! Transfert de notes et de dossiers vers un autre profil : ce qu'il faut copier dans la base
//! cible, puis supprimer de la base d'origine.
//!
//! Les dossiers gardent leur arborescence ; les notes transférées seules arrivent dans le
//! dossier par défaut du profil cible.

use std::collections::{HashMap, HashSet};

use crate::folder::Folder;
use crate::note::Note;

/// Copie d'un dossier. `key` est son identifiant d'origine : il relie les sous-dossiers et
/// les notes à leur parent dans la base cible, où les identifiants changent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderCopy {
    /// Identifiant d'origine.
    pub key: i64,
    /// Parent d'origine, `None` pour un dossier transféré (il arrive à la racine).
    pub parent_key: Option<i64>,
    /// Nom.
    pub name: String,
    /// Couleur.
    pub color: Option<String>,
    /// Position sous son parent (perdue pour un dossier transféré).
    pub sort_order: Option<i64>,
    /// Date de création.
    pub created_at: String,
}

/// Copie d'une note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteCopy {
    /// Dossier d'origine, `None` pour une note transférée seule (elle arrive dans le dossier
    /// par défaut du profil cible).
    pub folder_key: Option<i64>,
    /// Titre.
    pub title: String,
    /// Contenu.
    pub content: String,
    /// Tags, tels que stockés (tableau JSON).
    pub tags: String,
    /// Couleur.
    pub color: Option<String>,
    /// Position dans son dossier (perdue pour une note transférée seule).
    pub sort_order: Option<i64>,
    /// Date de création.
    pub created_at: String,
    /// Date de dernière modification.
    pub updated_at: String,
}

/// Ce que reçoit la base cible : les dossiers (parents avant enfants), puis les notes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransferBundle {
    /// Dossiers à créer.
    pub folders: Vec<FolderCopy>,
    /// Notes à créer.
    pub notes: Vec<NoteCopy>,
}

impl TransferBundle {
    /// Rien à copier.
    pub fn is_empty(&self) -> bool {
        self.folders.is_empty() && self.notes.is_empty()
    }
}

/// Le transfert prévu.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransferPlan {
    /// À copier dans la base cible.
    pub bundle: TransferBundle,
    /// Dossiers transférés, à supprimer (avec tout leur contenu) de la base d'origine.
    pub root_folders: Vec<i64>,
    /// Notes transférées seules, à supprimer de la base d'origine.
    pub loose_notes: Vec<i64>,
    /// Tous les dossiers qui partent (sous-dossiers compris).
    pub moved_folders: Vec<i64>,
    /// Toutes les notes qui partent (celles des dossiers comprises).
    pub moved_notes: Vec<i64>,
}

/// Prévoit le transfert des dossiers `folder_ids` (avec tout leur contenu) et des notes
/// `note_ids`. Le dossier par défaut et les identifiants inconnus sont ignorés ; un élément
/// déjà compris dans un dossier transféré ne l'est pas une seconde fois.
pub fn plan_transfer(
    folders: &[Folder],
    notes: &[Note],
    folder_ids: &[i64],
    note_ids: &[i64],
) -> TransferPlan {
    let tree = FolderTree::new(folders);
    let root_folders = tree.top_level_roots(folder_ids);
    let ordered = tree.subtrees(&root_folders);
    let moved: HashSet<i64> = ordered.iter().map(|folder| folder.id).collect();

    let inside: Vec<&Note> = notes
        .iter()
        .filter(|note| note.folder_id.is_some_and(|id| moved.contains(&id)))
        .collect();
    let loose: Vec<&Note> = unique_notes(notes, note_ids)
        .into_iter()
        .filter(|note| !note.folder_id.is_some_and(|id| moved.contains(&id)))
        .collect();

    let roots: HashSet<i64> = root_folders.iter().copied().collect();
    TransferPlan {
        bundle: TransferBundle {
            folders: ordered.iter().map(|f| folder_copy(f, &roots)).collect(),
            notes: inside
                .iter()
                .map(|note| note_copy(note, true))
                .chain(loose.iter().map(|note| note_copy(note, false)))
                .collect(),
        },
        root_folders,
        loose_notes: loose.iter().map(|note| note.id).collect(),
        moved_folders: ordered.iter().map(|folder| folder.id).collect(),
        moved_notes: inside.iter().chain(&loose).map(|note| note.id).collect(),
    }
}

/// Les notes demandées qui existent, chacune une fois, dans l'ordre de la demande.
fn unique_notes<'a>(notes: &'a [Note], note_ids: &[i64]) -> Vec<&'a Note> {
    let by_id: HashMap<i64, &Note> = notes.iter().map(|note| (note.id, note)).collect();
    let mut seen = HashSet::new();

    note_ids
        .iter()
        .filter(|id| seen.insert(**id))
        .filter_map(|id| by_id.get(id).copied())
        .collect()
}

fn folder_copy(folder: &Folder, roots: &HashSet<i64>) -> FolderCopy {
    let is_root = roots.contains(&folder.id);
    FolderCopy {
        key: folder.id,
        parent_key: if is_root { None } else { folder.parent_id },
        name: folder.name.clone(),
        color: folder.color.clone(),
        sort_order: if is_root { None } else { folder.sort_order },
        created_at: folder.created_at.clone(),
    }
}

/// `keeps_folder` : la note suit son dossier (transféré avec elle), sinon elle part seule.
fn note_copy(note: &Note, keeps_folder: bool) -> NoteCopy {
    NoteCopy {
        folder_key: note.folder_id.filter(|_| keeps_folder),
        title: note.title.clone(),
        content: note.content.clone(),
        tags: note.tags.clone(),
        color: note.color.clone(),
        sort_order: note.sort_order.filter(|_| keeps_folder),
        created_at: note.created_at.clone(),
        updated_at: note.updated_at.clone(),
    }
}

/// Arborescence des dossiers, indexée par identifiant.
struct FolderTree<'a> {
    by_id: HashMap<i64, &'a Folder>,
    children: HashMap<i64, Vec<&'a Folder>>,
}

impl<'a> FolderTree<'a> {
    fn new(folders: &'a [Folder]) -> Self {
        let mut children: HashMap<i64, Vec<&Folder>> = HashMap::new();
        for folder in folders {
            if let Some(parent) = folder.parent_id {
                children.entry(parent).or_default().push(folder);
            }
        }
        Self {
            by_id: folders.iter().map(|folder| (folder.id, folder)).collect(),
            children,
        }
    }

    /// Les dossiers demandés transférables (connus, pas le dossier par défaut) dont aucun
    /// ancêtre n'est aussi demandé, sans doublon, dans l'ordre de la demande.
    fn top_level_roots(&self, requested: &[i64]) -> Vec<i64> {
        let wanted: HashSet<i64> = requested
            .iter()
            .copied()
            .filter(|id| self.by_id.get(id).is_some_and(|f| !f.is_default))
            .collect();
        let mut seen = HashSet::new();

        requested
            .iter()
            .copied()
            .filter(|id| wanted.contains(id) && !self.has_ancestor_in(*id, &wanted))
            .filter(|id| seen.insert(*id))
            .collect()
    }

    /// Un ancêtre de `id` est dans `set` ; s'arrête sur un cycle de parents.
    fn has_ancestor_in(&self, id: i64, set: &HashSet<i64>) -> bool {
        let mut visited = HashSet::from([id]);
        let mut current = self.by_id.get(&id).and_then(|folder| folder.parent_id);

        while let Some(parent) = current {
            if !visited.insert(parent) {
                return false;
            }
            if set.contains(&parent) {
                return true;
            }
            current = self.by_id.get(&parent).and_then(|folder| folder.parent_id);
        }
        false
    }

    /// Les racines et tous leurs descendants, en largeur : un parent précède ses enfants.
    fn subtrees(&self, roots: &[i64]) -> Vec<&'a Folder> {
        let mut ordered: Vec<&Folder> = roots
            .iter()
            .filter_map(|id| self.by_id.get(id).copied())
            .collect();
        let mut seen: HashSet<i64> = ordered.iter().map(|folder| folder.id).collect();
        let mut next = 0;

        while let Some(folder) = ordered.get(next).copied() {
            next += 1;
            let children = self.children.get(&folder.id).into_iter().flatten();
            ordered.extend(children.filter(|child| seen.insert(child.id)));
        }
        ordered
    }
}

#[cfg(test)]
mod tests;
