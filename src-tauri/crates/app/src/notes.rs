//! Cas d'usage des notes.

pub mod tags;

use std::sync::Arc;

use domain::color::normalize_color;
use domain::note::{NewNote, Note, NoteChanges};
use domain::search::{fts_prefix_query, tag_like_pattern};

use crate::AppResult;
use crate::ports::{FolderStore, NoteStore};

/// Notes : création, modification, classement, recherche.
pub struct NotesService {
    notes: Arc<dyn NoteStore>,
    folders: Arc<dyn FolderStore>,
}

impl NotesService {
    /// Service branché sur ses stockages.
    pub fn new(notes: Arc<dyn NoteStore>, folders: Arc<dyn FolderStore>) -> Self {
        Self { notes, folders }
    }

    /// Toutes les notes, les plus récemment modifiées d'abord.
    pub fn all(&self) -> AppResult<Vec<Note>> {
        self.notes.all()
    }

    /// Une note.
    pub fn by_id(&self, id: i64) -> AppResult<Option<Note>> {
        self.notes.by_id(id)
    }

    /// Crée une note ; un dossier qui n'existe pas (plus) la place à la racine plutôt que
    /// de faire échouer la création.
    pub fn create(&self, mut note: NewNote) -> AppResult<Note> {
        note.folder_id = self.existing_folder(note.folder_id)?;
        self.notes.insert(&note)
    }

    /// Modifie une note ; sans champ, la rend telle quelle.
    pub fn update(&self, id: i64, changes: &NoteChanges) -> AppResult<Option<Note>> {
        if !changes.is_empty() {
            self.notes.update(id, changes)?;
        }
        self.notes.by_id(id)
    }

    /// Range la note dans un dossier ; un dossier inconnu laisse la note où elle est.
    pub fn move_to_folder(&self, id: i64, folder_id: Option<i64>) -> AppResult<Option<Note>> {
        if folder_id.is_some() && self.existing_folder(folder_id)?.is_none() {
            return self.notes.by_id(id);
        }
        self.notes.set_folder(id, folder_id)?;
        self.notes.by_id(id)
    }

    /// Range la note dans `folder_id` à la position donnée par `ordered_ids` (les notes du
    /// dossier dans l'ordre d'affichage).
    pub fn place(
        &self,
        id: i64,
        folder_id: Option<i64>,
        ordered_ids: &[i64],
    ) -> AppResult<Option<Note>> {
        let moved = self.move_to_folder(id, folder_id)?;

        if moved
            .as_ref()
            .is_some_and(|note| note.folder_id == folder_id)
        {
            self.notes.set_positions(folder_id, ordered_ids)?;
            return self.notes.by_id(id);
        }
        Ok(moved)
    }

    /// Supprime une note.
    pub fn delete(&self, id: i64) -> AppResult<()> {
        self.notes.delete(id)
    }

    /// Colore des notes ; une couleur invalide les décolore.
    pub fn set_color(&self, ids: &[i64], color: Option<&str>) -> AppResult<()> {
        if ids.is_empty() {
            return Ok(());
        }
        self.notes.set_color(ids, normalize_color(color).as_deref())
    }

    /// Recherche plein texte ; une recherche vide rend toutes les notes.
    pub fn search(&self, query: &str) -> AppResult<Vec<Note>> {
        match fts_prefix_query(query) {
            Some(fts_query) => self.notes.search(&fts_query),
            None => self.notes.all(),
        }
    }

    /// Notes portant un tag.
    pub fn by_tag(&self, tag: &str) -> AppResult<Vec<Note>> {
        self.notes.by_tag(&tag_like_pattern(tag))
    }

    /// Tous les tags utilisés, triés, sans doublon ; une colonne illisible est ignorée.
    pub fn all_tags(&self) -> AppResult<Vec<String>> {
        let columns = self.notes.tag_columns()?;
        let mut tags: Vec<String> = columns
            .iter()
            .flat_map(|column| tags::decode(column))
            .collect();

        tags.sort();
        tags.dedup();
        Ok(tags)
    }

    fn existing_folder(&self, folder_id: Option<i64>) -> AppResult<Option<i64>> {
        let Some(id) = folder_id else {
            return Ok(None);
        };
        Ok(self.folders.by_id(id)?.map(|folder| folder.id))
    }
}

#[cfg(test)]
mod tests;
