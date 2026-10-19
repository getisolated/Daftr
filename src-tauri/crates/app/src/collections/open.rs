//! Chemins donnés par Windows (association de fichiers, second lancement) ou déposés sur
//! la fenêtre.

use std::path::{Path, PathBuf};

use domain::collection::{
    Collection, CollectionFileRef, CollectionKind, is_markdown_file, relative_path,
};
use platform::paths::canonical_path;

use super::{CollectionsService, file_name};
use crate::AppResult;

impl CollectionsService {
    /// Les dossiers et fichiers markdown deviennent des collections ; les fichiers sont
    /// rendus pour être ouverts (un fichier d'un dossier déjà ouvert s'ouvre depuis lui).
    pub fn open_paths(&self, paths: &[PathBuf]) -> AppResult<Vec<CollectionFileRef>> {
        let mut to_open = Vec::with_capacity(paths.len());

        for candidate in paths {
            to_open.extend(self.open_path(&canonical_path(candidate))?);
        }
        Ok(to_open)
    }

    fn open_path(&self, absolute: &Path) -> AppResult<Option<CollectionFileRef>> {
        if is_markdown_file(&file_name(absolute))
            && let Some(inside) = self.file_in_open_folder(absolute)?
        {
            return Ok(Some(inside));
        }
        let opened = self.add(absolute)?;

        Ok(opened
            .filter(|collection| collection.kind == CollectionKind::File)
            .map(|file| {
                let path = file_name(Path::new(&file.path));
                CollectionFileRef {
                    root: file.path,
                    path,
                }
            }))
    }

    fn file_in_open_folder(&self, absolute: &Path) -> AppResult<Option<CollectionFileRef>> {
        let folders = self.store.all()?;
        Ok(folders
            .iter()
            .find_map(|collection| file_ref_inside(collection, absolute)))
    }
}

fn file_ref_inside(collection: &Collection, absolute: &Path) -> Option<CollectionFileRef> {
    if collection.kind != CollectionKind::Folder {
        return None;
    }
    let path =
        relative_path(Path::new(&collection.path), absolute).filter(|path| !path.is_empty())?;
    Some(CollectionFileRef {
        root: collection.path.clone(),
        path,
    })
}
