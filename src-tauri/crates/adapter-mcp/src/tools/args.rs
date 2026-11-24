//! Paramètres des outils. Leurs descriptions, en anglais, forment le schéma JSON montré
//! aux IA.

use schemars::JsonSchema;
use serde::Deserialize;

/// Paramètres de `list_notes`.
#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListNotesArgs {
    /// Seulement les notes de ce dossier.
    #[schemars(description = "Only the notes directly in this folder (id from list_folders).")]
    #[serde(default)]
    pub folder_id: Option<i64>,
    /// Seulement les notes portant ce tag.
    #[schemars(description = "Only the notes carrying this tag (exact name, see list_tags).")]
    #[serde(default)]
    pub tag: Option<String>,
    /// Nombre maximal de notes.
    #[schemars(
        description = "Maximum number of notes to return, most recently updated first. Default 50, at most 500.",
        range(min = 1, max = 500)
    )]
    #[serde(default)]
    pub limit: Option<u32>,
}

/// Paramètres de `search_notes`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct SearchNotesArgs {
    /// Mots cherchés.
    #[schemars(
        description = "Words to look for in the titles and contents of the notes. Every word must match; the last one also matches as a prefix."
    )]
    pub query: String,
    /// Nombre maximal de résultats.
    #[schemars(
        description = "Maximum number of notes to return, best matches first. Default 20, at most 500.",
        range(min = 1, max = 500)
    )]
    #[serde(default)]
    pub limit: Option<u32>,
}

/// Paramètres de `get_note`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GetNoteArgs {
    /// Identifiant de la note.
    #[schemars(description = "Id of the note (from list_notes or search_notes).")]
    pub id: i64,
    /// Seulement la section de ce titre.
    #[schemars(
        description = "Read only the section under this heading (its text, case-insensitive, '#' optional), subsections included. Cannot be combined with from_line/to_line."
    )]
    #[serde(default)]
    pub section: Option<String>,
    /// Première ligne lue.
    #[schemars(
        description = "First line to read, counted from 1. Default 1.",
        range(min = 1)
    )]
    #[serde(default)]
    pub from_line: Option<u32>,
    /// Dernière ligne lue.
    #[schemars(
        description = "Last line to read, inclusive. Default: the end of the note.",
        range(min = 1)
    )]
    #[serde(default)]
    pub to_line: Option<u32>,
}

/// Paramètres de `get_notes`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct GetNotesArgs {
    /// Identifiants des notes.
    #[schemars(description = "Ids of the notes to read, at most 20.")]
    pub ids: Vec<i64>,
}

/// Paramètres de `create_note`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreateNoteArgs {
    /// Texte markdown.
    #[schemars(description = "Markdown content of the note.")]
    pub content: String,
    /// Titre.
    #[schemars(
        description = "Title of the note. Daftr titles a note with its first line: when the content does not already start with this title, a '# title' heading is added at the top. Without it, the first line of the content is the title."
    )]
    #[serde(default)]
    pub title: Option<String>,
    /// Dossier.
    #[schemars(
        description = "Folder to create the note in (id from list_folders). Defaults to the Inbox folder."
    )]
    #[serde(default)]
    pub folder_id: Option<i64>,
    /// Tags.
    #[schemars(description = "Tags of the note.")]
    #[serde(default)]
    pub tags: Option<Vec<String>>,
}

/// Paramètres de `update_note`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateNoteArgs {
    /// Identifiant de la note.
    #[schemars(description = "Id of the note to update.")]
    pub id: i64,
    /// Nouveau texte.
    #[schemars(
        description = "New markdown content, replacing the whole note. Its first line becomes the title."
    )]
    pub content: String,
}

/// Paramètres de `edit_note`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct EditNoteArgs {
    /// Identifiant de la note.
    #[schemars(description = "Id of the note to edit.")]
    pub id: i64,
    /// Passage remplacé.
    #[schemars(
        description = "Exact text to replace, whitespace included. Must appear once in the note unless replace_all is set: add surrounding text to make it unique."
    )]
    pub old_text: String,
    /// Passage mis à la place.
    #[schemars(description = "Text put in its place; empty to delete the passage.")]
    pub new_text: String,
    /// Remplacer toutes les occurrences.
    #[schemars(description = "Replace every occurrence of old_text. Default false.")]
    #[serde(default)]
    pub replace_all: Option<bool>,
}

/// Paramètres de `append_to_note`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AppendToNoteArgs {
    /// Identifiant de la note.
    #[schemars(description = "Id of the note to append to.")]
    pub id: i64,
    /// Texte ajouté.
    #[schemars(description = "Markdown text added at the end of the note, after a blank line.")]
    pub text: String,
}
