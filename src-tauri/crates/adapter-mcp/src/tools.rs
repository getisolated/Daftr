//! Outils exposés aux IA : lecture et écriture des notes, sans suppression.
//!
//! Chaque outil est une fonction synchrone (comme les services) qui rend le JSON à renvoyer,
//! ou un message d'erreur lisible par l'IA. Les réponses restent courtes : chaque octet est
//! payé en tokens par l'IA.

mod args;
mod read;
mod reads;

use std::collections::HashMap;
use std::sync::Arc;

use app::AppError;
use app::folders::FoldersService;
use app::notes::NotesService;
use domain::folder::Folder;
use domain::links::rewrite_note_links;
use domain::note::edit::{ReplaceError, replace_passage};
use domain::note::{
    NewNote, Note, NoteChanges, NoteHead, append_text, content_with_title, extract_title,
};
use domain::search::snippet;
use domain::tags::normalize_tags;
use serde::Serialize;
use serde_json::{Value, json};

pub(crate) use args::{
    AppendToNoteArgs, CreateNoteArgs, EditNoteArgs, GetNoteArgs, GetNotesArgs, ListNotesArgs,
    SearchNotesArgs, UpdateNoteArgs,
};
pub(crate) use read::{get_note, get_notes};
pub(crate) use reads::KnownContents;

/// Nombre de notes rendues par `list_notes` sans limite demandée.
const DEFAULT_LIST_LIMIT: usize = 50;
/// Nombre de notes rendues par `search_notes` sans limite demandée.
const DEFAULT_SEARCH_LIMIT: usize = 20;
/// Plafond de toute limite demandée.
const MAX_LIMIT: usize = 500;
/// Longueur des extraits de `search_notes`, en caractères.
const SNIPPET_LENGTH: usize = 200;
/// Longueur d'une date rendue : `YYYY-MM-DD HH:MM`, les secondes n'apportent rien.
const DATE_LENGTH: usize = 16;

/// Services de l'application partagés avec le serveur MCP.
#[derive(Clone)]
pub struct McpServices {
    /// Notes.
    pub notes: Arc<NotesService>,
    /// Dossiers.
    pub folders: Arc<FoldersService>,
}

/// D'où viennent les services à chaque appel d'outil : ceux du profil actif, qui change
/// sans redémarrer le serveur (les IA connectées gardent leur session).
pub trait McpServicesSource: Send + Sync {
    /// Services du profil actif, `None` tant qu'il est verrouillé.
    fn current(&self) -> Option<McpServices>;
}

/// Des services fixes (une seule base).
impl McpServicesSource for McpServices {
    fn current(&self) -> Option<McpServices> {
        Some(self.clone())
    }
}

/// Résultat d'un outil : le JSON à renvoyer, ou un message d'erreur pour l'IA.
pub(crate) type ToolResult = Result<Value, String>;

/// Une note sans son contenu, réduite à ce qui aide à choisir : pas de date de création,
/// pas de tags vides.
#[derive(Debug, Serialize)]
struct NoteSummary {
    id: i64,
    title: String,
    folder_id: Option<i64>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tags: Vec<String>,
    updated_at: String,
}

impl From<&Note> for NoteSummary {
    fn from(note: &Note) -> Self {
        Self {
            id: note.id,
            title: note.title.clone(),
            folder_id: note.folder_id,
            tags: app::notes::tags::decode(&note.tags),
            updated_at: short_date(&note.updated_at),
        }
    }
}

impl From<&NoteHead> for NoteSummary {
    fn from(head: &NoteHead) -> Self {
        Self {
            id: head.id,
            title: head.title.clone(),
            folder_id: head.folder_id,
            tags: app::notes::tags::decode(&head.tags),
            updated_at: short_date(&head.updated_at),
        }
    }
}

/// Un dossier et son chemin depuis la racine.
#[derive(Debug, Serialize)]
struct FolderEntry {
    id: i64,
    name: String,
    parent_id: Option<i64>,
    path: String,
    is_default: bool,
}

/// Tous les dossiers, avec leur chemin (`Parent/Enfant`).
pub(crate) fn list_folders(services: &McpServices) -> ToolResult {
    let folders = services.folders.all().map_err(failure)?;
    let by_id: HashMap<i64, &Folder> = folders.iter().map(|folder| (folder.id, folder)).collect();
    let mut entries: Vec<FolderEntry> = folders
        .iter()
        .map(|folder| FolderEntry {
            id: folder.id,
            name: folder.name.clone(),
            parent_id: folder.parent_id,
            path: folder_path(folder, &by_id),
            is_default: folder.is_default,
        })
        .collect();

    entries.sort_by_cached_key(|entry| entry.path.to_lowercase());
    Ok(json!({ "folders": entries }))
}

/// Les notes, les plus récemment modifiées d'abord, filtrées par dossier et tag ; la base ne
/// rend que les premières, et sans leur contenu.
pub(crate) fn list_notes(services: &McpServices, args: ListNotesArgs) -> ToolResult {
    if let Some(folder_id) = args.folder_id {
        ensure_folder(services, folder_id)?;
    }
    let tag = args
        .tag
        .as_deref()
        .map(str::trim)
        .filter(|tag| !tag.is_empty());
    let page = services
        .notes
        .heads(args.folder_id, tag, limit(args.limit, DEFAULT_LIST_LIMIT))
        .map_err(failure)?;
    let listed: Vec<NoteSummary> = page.items.iter().map(NoteSummary::from).collect();

    Ok(json!({ "total": page.total, "notes": listed }))
}

/// Recherche plein texte, avec un extrait autour des mots trouvés.
pub(crate) fn search_notes(services: &McpServices, args: SearchNotesArgs) -> ToolResult {
    let query = args.query.trim();
    if query.is_empty() {
        return Err("The query must not be empty.".to_owned());
    }
    let page = services
        .notes
        .search_page(query, limit(args.limit, DEFAULT_SEARCH_LIMIT))
        .map_err(failure)?;
    let results: Vec<Value> = page
        .items
        .iter()
        .map(|note| {
            let mut entry = json!(NoteSummary::from(note));
            entry["snippet"] = json!(snippet(&note.content, query, SNIPPET_LENGTH));
            entry
        })
        .collect();

    Ok(json!({ "total": page.total, "notes": results }))
}

/// Tous les tags utilisés.
pub(crate) fn list_tags(services: &McpServices) -> ToolResult {
    Ok(json!({ "tags": services.notes.all_tags().map_err(failure)? }))
}

/// Crée une note ; sans dossier, elle arrive dans le dossier par défaut (Inbox).
pub(crate) fn create_note(services: &McpServices, args: CreateNoteArgs) -> ToolResult {
    let folder_id = match args.folder_id {
        Some(id) => Some(ensure_folder(services, id)?),
        None => services.folders.default_folder_id().map_err(failure)?,
    };
    let content = match args.title.as_deref() {
        Some(title) => content_with_title(title, &args.content),
        None => args.content,
    };
    let content = with_uuid_links(services, &content).unwrap_or(content);
    let note = NewNote {
        title: extract_title(&content),
        content,
        tags: normalize_tags(&args.tags.unwrap_or_default()),
        folder_id,
    };

    let created = services.notes.create(note).map_err(failure)?;
    Ok(json!(NoteSummary::from(&created)))
}

/// Remplace le contenu d'une note (et donc son titre, sa première ligne). Le contenu est
/// rejoué sur la version lue par l'IA : ce qui a changé depuis ailleurs est gardé.
pub(crate) fn update_note(
    services: &McpServices,
    known: &KnownContents,
    args: UpdateNoteArgs,
) -> ToolResult {
    let note = existing_note(services, args.id)?;
    let content = with_uuid_links(services, &args.content).unwrap_or(args.content);
    let base = known.of(&note.uuid);
    // Le contenu envoyé est aussi ce que l'IA connaît désormais de la note
    let updated = replace_content(services, args.id, content.clone(), base)?;
    known.remember(note.uuid, content);
    Ok(json!(NoteSummary::from(&updated)))
}

/// Remplace un passage exact d'une note, le reste du texte restant intact. Le passage est
/// cherché dans la version actuelle, qui sert aussi de base : ce qui change ailleurs en même
/// temps (autre appareil, éditeur) est gardé.
pub(crate) fn edit_note(
    services: &McpServices,
    known: &KnownContents,
    args: EditNoteArgs,
) -> ToolResult {
    let note = existing_note(services, args.id)?;
    let replace_all = args.replace_all.unwrap_or(false);
    let edited = replace_passage(&note.content, &args.old_text, &args.new_text, replace_all)
        .map_err(|error| edit_failure(error, args.id))?;
    let content = with_uuid_links(services, &edited).unwrap_or(edited);
    let updated = replace_content(services, args.id, content.clone(), Some(note.content))?;
    known.remember(note.uuid, content);
    Ok(json!(NoteSummary::from(&updated)))
}

/// Ajoute du texte à la fin d'une note, après une ligne vide.
pub(crate) fn append_to_note(
    services: &McpServices,
    known: &KnownContents,
    args: AppendToNoteArgs,
) -> ToolResult {
    if args.text.trim().is_empty() {
        return Err("The text to append must not be empty.".to_owned());
    }
    let note = existing_note(services, args.id)?;
    let appended = append_text(&note.content, &args.text);
    let content = with_uuid_links(services, &appended).unwrap_or(appended);
    let updated = replace_content(services, args.id, content.clone(), Some(note.content))?;
    known.remember(note.uuid, content);
    Ok(json!(NoteSummary::from(&updated)))
}

/// Écrit le contenu (déjà en liens `note://<uuid>`) construit sur `base`.
fn replace_content(
    services: &McpServices,
    id: i64,
    content: String,
    base: Option<String>,
) -> Result<Note, String> {
    let changes = NoteChanges {
        title: Some(extract_title(&content)),
        content: Some(content),
        tags: None,
        base,
    };
    let updated = services.notes.update(id, &changes).map_err(failure)?;
    updated.ok_or_else(|| note_not_found(id))
}

/// Les liens `note://<id>` que l'IA écrit avec les identifiants qu'elle voit, réécrits avec
/// l'uuid de la note visée : seul l'uuid est le même sur tous les appareils.
fn with_uuid_links(services: &McpServices, content: &str) -> Option<String> {
    let uuid_of = |id: i64| {
        services
            .notes
            .by_id(id)
            .ok()
            .flatten()
            .map(|note| note.uuid)
    };
    rewrite_note_links(content, uuid_of)
}

/// Une note avec son contenu, son uuid et sa date de création, que les listes ne portent pas.
fn with_content(note: &Note) -> Value {
    let mut value = json!(NoteSummary::from(note));
    value["uuid"] = json!(note.uuid.to_string());
    value["created_at"] = json!(short_date(&note.created_at));
    value["content"] = json!(note.content);
    value
}

fn existing_note(services: &McpServices, id: i64) -> Result<Note, String> {
    services
        .notes
        .by_id(id)
        .map_err(failure)?
        .ok_or_else(|| note_not_found(id))
}

fn ensure_folder(services: &McpServices, id: i64) -> Result<i64, String> {
    let folders = services.folders.all().map_err(failure)?;
    if folders.iter().any(|folder| folder.id == id) {
        Ok(id)
    } else {
        Err(format!(
            "Folder {id} not found. Use list_folders to get the existing folders."
        ))
    }
}

fn note_not_found(id: i64) -> String {
    format!("Note {id} not found. Use list_notes or search_notes to find notes.")
}

fn edit_failure(error: ReplaceError, id: i64) -> String {
    match error {
        ReplaceError::EmptyPassage => "old_text must not be empty.".to_owned(),
        ReplaceError::NotFound => format!(
            "old_text not found in note {id}: it must match exactly, whitespace included. \
             Use get_note to read the current text."
        ),
        ReplaceError::Ambiguous(count) => format!(
            "old_text appears {count} times in note {id}: add surrounding text to make it \
             unique, or set replace_all."
        ),
    }
}

/// Chemin du dossier depuis la racine ; s'arrête sur un parent inconnu ou un cycle.
fn folder_path(folder: &Folder, by_id: &HashMap<i64, &Folder>) -> String {
    let mut names = vec![folder.name.as_str()];
    let mut parent = folder.parent_id;

    while let Some(current) = parent.and_then(|id| by_id.get(&id)) {
        if names.len() > by_id.len() {
            break;
        }
        names.push(current.name.as_str());
        parent = current.parent_id;
    }
    names.reverse();
    names.join("/")
}

fn limit(requested: Option<u32>, default: usize) -> usize {
    requested
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or(default)
        .clamp(1, MAX_LIMIT)
}

/// La date sans ses secondes (`YYYY-MM-DD HH:MM`).
fn short_date(date: &str) -> String {
    date.chars().take(DATE_LENGTH).collect()
}

fn failure(error: AppError) -> String {
    tracing::warn!(%error, "outil MCP en échec");
    error.to_string()
}

#[cfg(test)]
mod tests;
