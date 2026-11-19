//! Transfert d'éléments du profil actif vers un autre profil : un déplacement.
//!
//! Tout est d'abord copié dans la base cible (en une transaction), puis supprimé de la base
//! d'origine : un échec de la copie ne fait rien perdre ; un échec de la suppression laisse
//! au pire l'élément dans les deux profils.

use domain::folder::FolderDeleteMode;
use domain::transfer::plan_transfer;

use crate::AppResult;
use crate::collections::CollectionsService;
use crate::ports::{CollectionStore, FolderStore, NoteImporter, NoteStore};

/// Ce que l'utilisateur transfère.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransferRequest {
    /// Notes (une note d'un dossier transféré part avec lui).
    pub note_ids: Vec<i64>,
    /// Dossiers, avec tout leur contenu.
    pub folder_ids: Vec<i64>,
    /// Collections, par chemin : seul leur enregistrement change de profil.
    pub collections: Vec<String>,
}

/// La base du profil actif.
pub struct TransferSource<'a> {
    /// Notes.
    pub notes: &'a dyn NoteStore,
    /// Dossiers.
    pub folders: &'a dyn FolderStore,
    /// Collections (le service arrête aussi de surveiller celles qui partent).
    pub collections: &'a CollectionsService,
}

/// La base du profil qui reçoit.
pub struct TransferTarget<'a> {
    /// Écriture des notes et dossiers.
    pub importer: &'a dyn NoteImporter,
    /// Liste des collections.
    pub collections: &'a dyn CollectionStore,
}

/// Ce qui a quitté le profil actif (le renderer ferme leurs onglets).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Transferred {
    /// Notes, celles des dossiers comprises.
    pub note_ids: Vec<i64>,
    /// Dossiers, sous-dossiers compris.
    pub folder_ids: Vec<i64>,
    /// Collections.
    pub collections: Vec<String>,
}

/// Déplace les éléments demandés vers le profil cible.
pub fn transfer(
    source: &TransferSource<'_>,
    target: &TransferTarget<'_>,
    request: &TransferRequest,
) -> AppResult<Transferred> {
    let mut transferred = transfer_notes(source, target.importer, request)?;

    for root in &request.collections {
        if source.collections.transfer_to(root, target.collections)? {
            transferred.collections.push(root.clone());
        }
    }
    Ok(transferred)
}

fn transfer_notes(
    source: &TransferSource<'_>,
    importer: &dyn NoteImporter,
    request: &TransferRequest,
) -> AppResult<Transferred> {
    if request.note_ids.is_empty() && request.folder_ids.is_empty() {
        return Ok(Transferred::default());
    }
    let (folders, notes) = (source.folders.all()?, source.notes.all()?);
    let plan = plan_transfer(&folders, &notes, &request.folder_ids, &request.note_ids);
    if plan.bundle.is_empty() {
        return Ok(Transferred::default());
    }

    importer.import(&plan.bundle)?;
    for id in &plan.root_folders {
        source.folders.delete(*id, FolderDeleteMode::DeleteAll)?;
    }
    for id in &plan.loose_notes {
        source.notes.delete(*id)?;
    }
    Ok(Transferred {
        note_ids: plan.moved_notes,
        folder_ids: plan.moved_folders,
        collections: Vec::new(),
    })
}

#[cfg(test)]
mod tests;
