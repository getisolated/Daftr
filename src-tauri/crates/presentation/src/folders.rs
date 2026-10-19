//! Commandes des dossiers.

use app::folders::FoldersService;
use domain::folder::FolderDeleteMode;

use crate::IpcResult;
use crate::dto::FolderDto;

/// Tous les dossiers.
pub fn all(folders: &FoldersService) -> IpcResult<Vec<FolderDto>> {
    Ok(folders.all()?.into_iter().map(FolderDto::from).collect())
}

/// Crée un dossier ; `null` si refusé.
pub fn create(
    folders: &FoldersService,
    name: &str,
    parent_id: Option<i64>,
) -> IpcResult<Option<FolderDto>> {
    Ok(folders.create(name, parent_id)?.map(FolderDto::from))
}

/// Renomme un dossier ; `null` si refusé.
pub fn rename(folders: &FoldersService, id: i64, name: &str) -> IpcResult<Option<FolderDto>> {
    Ok(folders.rename(id, name)?.map(FolderDto::from))
}

/// Déplace un dossier ; `null` si refusé.
pub fn move_to(
    folders: &FoldersService,
    id: i64,
    parent_id: Option<i64>,
) -> IpcResult<Option<FolderDto>> {
    Ok(folders.move_folder(id, parent_id)?.map(FolderDto::from))
}

/// Range un dossier à une position ; `null` si refusé.
pub fn place(
    folders: &FoldersService,
    id: i64,
    parent_id: Option<i64>,
    ordered_ids: &[i64],
) -> IpcResult<Option<FolderDto>> {
    Ok(folders
        .place(id, parent_id, ordered_ids)?
        .map(FolderDto::from))
}

/// Supprime un dossier ; tout mode autre qu'un `deleteAll` explicite garde les notes. Rend
/// les notes supprimées, `null` si refusé.
pub fn delete(
    folders: &FoldersService,
    id: i64,
    mode: Option<&str>,
) -> IpcResult<Option<Vec<i64>>> {
    Ok(folders.delete(id, FolderDeleteMode::from_request(mode))?)
}

/// Vide un dossier ; rend les notes supprimées, `null` pour un dossier inconnu.
pub fn empty(folders: &FoldersService, id: i64) -> IpcResult<Option<Vec<i64>>> {
    Ok(folders.empty(id)?)
}

/// Colore des dossiers.
pub fn set_color(folders: &FoldersService, ids: &[i64], color: Option<&str>) -> IpcResult<bool> {
    folders.set_color(ids, color)?;
    Ok(true)
}
