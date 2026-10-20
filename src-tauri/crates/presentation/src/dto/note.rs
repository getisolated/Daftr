//! Notes.

use domain::export::ExportedNote;
use domain::note::{Note, NoteChanges};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Une note (ligne de la table `notes`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct NoteDto {
    /// Identifiant.
    #[ts(type = "number")]
    pub id: i64,
    /// Titre.
    pub title: String,
    /// Texte markdown.
    pub content: String,
    /// Tags, tableau JSON encodé.
    pub tags: String,
    /// Dossier, `null` à la racine.
    #[ts(type = "number | null")]
    pub folder_id: Option<i64>,
    /// Position dans le dossier.
    #[ts(type = "number | null")]
    pub sort_order: Option<i64>,
    /// Couleur `#rrggbb`.
    pub color: Option<String>,
    /// Date de création.
    pub created_at: String,
    /// Date de modification.
    pub updated_at: String,
}

impl From<Note> for NoteDto {
    fn from(note: Note) -> Self {
        Self {
            id: note.id,
            title: note.title,
            content: note.content,
            tags: note.tags,
            folder_id: note.folder_id,
            sort_order: note.sort_order,
            color: note.color,
            created_at: note.created_at,
            updated_at: note.updated_at,
        }
    }
}

/// Champs modifiés d'une note (absents = inchangés).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, TS)]
#[ts(export)]
pub struct NoteChangesDto {
    /// Nouveau titre.
    #[ts(optional)]
    pub title: Option<String>,
    /// Nouveau contenu.
    #[ts(optional)]
    pub content: Option<String>,
    /// Nouveaux tags, tableau JSON encodé.
    #[ts(optional)]
    pub tags: Option<String>,
}

impl From<NoteChangesDto> for NoteChanges {
    fn from(changes: NoteChangesDto) -> Self {
        Self {
            title: changes.title,
            content: changes.content,
            tags: changes.tags,
        }
    }
}

/// Une note d'un export de dossier.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, TS)]
#[ts(export)]
pub struct ExportedNoteDto {
    /// Chemin sous le dossier exporté (dossiers puis titre, sans extension).
    pub path: String,
    /// Texte markdown.
    pub content: String,
}

impl From<ExportedNoteDto> for ExportedNote {
    fn from(note: ExportedNoteDto) -> Self {
        Self {
            path: note.path,
            content: note.content,
        }
    }
}
