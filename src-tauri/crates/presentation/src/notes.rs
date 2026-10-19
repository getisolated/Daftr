//! Commandes des notes.

use app::notes::NotesService;
use domain::note::NewNote;

use crate::IpcResult;
use crate::dto::{NoteChangesDto, NoteDto};

/// Toutes les notes.
pub fn all(notes: &NotesService) -> IpcResult<Vec<NoteDto>> {
    Ok(notes.all()?.into_iter().map(NoteDto::from).collect())
}

/// Une note, `null` si elle n'existe pas.
pub fn by_id(notes: &NotesService, id: i64) -> IpcResult<Option<NoteDto>> {
    Ok(notes.by_id(id)?.map(NoteDto::from))
}

/// Crée une note.
pub fn create(
    notes: &NotesService,
    title: String,
    content: String,
    tags: Option<Vec<String>>,
    folder_id: Option<i64>,
) -> IpcResult<NoteDto> {
    let note = NewNote {
        title,
        content,
        tags: tags.unwrap_or_default(),
        folder_id,
    };
    Ok(notes.create(note)?.into())
}

/// Modifie une note.
pub fn update(
    notes: &NotesService,
    id: i64,
    changes: NoteChangesDto,
) -> IpcResult<Option<NoteDto>> {
    Ok(notes.update(id, &changes.into())?.map(NoteDto::from))
}

/// Range une note dans un dossier.
pub fn move_to(
    notes: &NotesService,
    id: i64,
    folder_id: Option<i64>,
) -> IpcResult<Option<NoteDto>> {
    Ok(notes.move_to_folder(id, folder_id)?.map(NoteDto::from))
}

/// Range une note à une position.
pub fn place(
    notes: &NotesService,
    id: i64,
    folder_id: Option<i64>,
    ordered_ids: &[i64],
) -> IpcResult<Option<NoteDto>> {
    Ok(notes.place(id, folder_id, ordered_ids)?.map(NoteDto::from))
}

/// Supprime une note.
pub fn delete(notes: &NotesService, id: i64) -> IpcResult<bool> {
    notes.delete(id)?;
    Ok(true)
}

/// Colore des notes.
pub fn set_color(notes: &NotesService, ids: &[i64], color: Option<&str>) -> IpcResult<bool> {
    notes.set_color(ids, color)?;
    Ok(true)
}

/// Recherche plein texte.
pub fn search(notes: &NotesService, query: &str) -> IpcResult<Vec<NoteDto>> {
    Ok(notes
        .search(query)?
        .into_iter()
        .map(NoteDto::from)
        .collect())
}

/// Notes portant un tag.
pub fn by_tag(notes: &NotesService, tag: &str) -> IpcResult<Vec<NoteDto>> {
    Ok(notes.by_tag(tag)?.into_iter().map(NoteDto::from).collect())
}

/// Tous les tags.
pub fn all_tags(notes: &NotesService) -> IpcResult<Vec<String>> {
    Ok(notes.all_tags()?)
}
