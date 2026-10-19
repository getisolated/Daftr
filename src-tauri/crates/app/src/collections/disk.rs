//! Opérations sur les fichiers d'une collection, toujours désignés par un chemin relatif
//! vérifié : rien en dehors d'une collection enregistrée n'est accessible.

use std::path::{Path, PathBuf};

use domain::collection::{
    Collection, CollectionKind, entry_name, image_mime, is_markdown_file, is_same_or_inside,
    relative_path, resolve_inside, with_markdown_extension,
};

use super::{CollectionsService, file_name};
use crate::ports::EntryKind;
use crate::{AppError, AppResult, Refusal};

impl CollectionsService {
    /// Lit un fichier de la collection.
    pub fn read(&self, root: &str, relative: &str) -> AppResult<String> {
        let (_, absolute) = self.resolve(root, relative)?;
        self.files.read_text(&absolute)
    }

    /// Lit une image de la collection (affichée par l'aperçu) ; rend son contenu et son type
    /// MIME. Tout autre fichier est refusé.
    pub fn read_image(&self, root: &str, relative: &str) -> AppResult<(Vec<u8>, &'static str)> {
        let (_, absolute) = self.resolve(root, relative)?;
        let mime = image_mime(&file_name(&absolute)).ok_or(Refusal::NotAnImage)?;

        Ok((self.files.read_bytes(&absolute)?, mime))
    }

    /// Écrit un fichier markdown de la collection.
    pub fn write(&self, root: &str, relative: &str, content: &str) -> AppResult<()> {
        let (_, absolute) = self.resolve(root, relative)?;

        if !is_markdown_file(&file_name(&absolute)) {
            return Err(Refusal::MarkdownOnly.into());
        }
        self.files.write_text(&absolute, content)
    }

    /// Crée un fichier markdown vide dans un dossier de la collection (`folder` relatif,
    /// vide pour la racine) ; rend son chemin relatif.
    pub fn create_file(&self, root: &str, folder: &str, name: &str) -> AppResult<String> {
        let collection = self.require(root)?;
        if collection.kind != CollectionKind::Folder {
            return Err(Refusal::CreateInFolderOnly.into());
        }
        let name = entry_name(name).ok_or(Refusal::InvalidName)?;
        let absolute = self
            .folder_path(&collection, folder)?
            .join(with_markdown_extension(&name));

        self.files.create_new(&absolute)?;
        to_relative(&collection, &absolute)
    }

    /// Renomme un fichier ou un dossier sur place ; rend son nouveau chemin relatif.
    pub fn rename_path(&self, root: &str, relative: &str, name: &str) -> AppResult<String> {
        let (collection, absolute) = self.resolve(root, relative)?;
        if collection.kind == CollectionKind::File {
            return Err(Refusal::RenameFromFolder.into());
        }
        let target = absolute.with_file_name(self.renamed(&absolute, name)?);

        self.move_entry(&absolute, &target)?;
        to_relative(&collection, &target)
    }

    /// Déplace un fichier ou un dossier dans un autre dossier de la même collection
    /// (`folder` vide pour la racine) ; rend son nouveau chemin relatif.
    pub fn move_path(&self, root: &str, relative: &str, folder: &str) -> AppResult<String> {
        let (collection, absolute) = self.resolve(root, relative)?;
        if collection.kind == CollectionKind::File {
            return Err(Refusal::RenameFromFolder.into());
        }
        let target = self
            .folder_path(&collection, folder)?
            .join(file_name(&absolute));

        if is_same_or_inside(&target, &absolute) {
            return Err(Refusal::MoveIntoItself.into());
        }
        self.move_entry(&absolute, &target)?;
        to_relative(&collection, &target)
    }

    /// Envoie un fichier ou un dossier à la corbeille.
    pub fn trash(&self, root: &str, relative: &str) -> AppResult<()> {
        let (_, absolute) = self.resolve(root, relative)?;
        self.files.trash(&absolute)
    }

    /// Chemin à montrer dans l'Explorateur : la collection, ou l'un de ses fichiers.
    pub fn reveal_target(&self, root: &str, relative: Option<&str>) -> AppResult<PathBuf> {
        match relative.filter(|relative| !relative.is_empty()) {
            Some(relative) => self.resolve(root, relative).map(|(_, absolute)| absolute),
            None => self
                .require(root)
                .map(|collection| PathBuf::from(collection.path)),
        }
    }

    /// La collection et le chemin absolu d'une de ses entrées ; tout chemin qui en sort est
    /// refusé. Une collection fichier n'a qu'une entrée : son propre nom.
    pub(super) fn resolve(&self, root: &str, relative: &str) -> AppResult<(Collection, PathBuf)> {
        let collection = self.require(root)?;
        let collection_path = PathBuf::from(&collection.path);

        let absolute = match collection.kind {
            CollectionKind::File => {
                (relative == file_name(&collection_path)).then_some(collection_path)
            }
            CollectionKind::Folder => resolve_inside(&collection_path, relative),
        };
        let absolute = absolute.ok_or(Refusal::OutsideCollection)?;
        Ok((collection, absolute))
    }

    fn folder_path(&self, collection: &Collection, folder: &str) -> AppResult<PathBuf> {
        if folder.is_empty() {
            return Ok(PathBuf::from(&collection.path));
        }
        self.resolve(&collection.path, folder)
            .map(|(_, absolute)| absolute)
    }

    /// Nouveau nom valide ; un fichier garde son extension markdown.
    fn renamed(&self, absolute: &Path, name: &str) -> AppResult<String> {
        let name = entry_name(name).ok_or(Refusal::InvalidName)?;

        match self.files.entry_kind(absolute) {
            Some(EntryKind::File) => Ok(with_markdown_extension(&name)),
            Some(EntryKind::Directory) => Ok(name),
            None => Err(AppError::file_system(format!(
                "{} does not exist",
                absolute.display()
            ))),
        }
    }

    fn move_entry(&self, from: &Path, to: &Path) -> AppResult<()> {
        if self.files.entry_kind(to).is_some() {
            return Err(Refusal::NameTaken.into());
        }
        self.files.rename(from, to)
    }
}

fn to_relative(collection: &Collection, absolute: &Path) -> AppResult<String> {
    relative_path(Path::new(&collection.path), absolute)
        .ok_or_else(|| Refusal::OutsideCollection.into())
}
