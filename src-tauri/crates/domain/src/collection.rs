//! Collections : dossiers et fichiers markdown du disque ouverts dans l'application.
//!
//! Leur contenu est lu sur le disque, jamais stocké ; seule la liste des collections l'est.

mod graph;
mod links;
mod media;
mod names;
mod path;
mod text_search;
mod tree;

pub use graph::{CollectionGraph, FileContent, GraphLink, GraphNode, build_collection_graph};
pub use links::{extract_link_targets, percent_decode, resolve_collection_link};
pub use media::image_mime;
pub use names::{MAX_DISPLAY_NAME_LENGTH, display_name, entry_name, with_markdown_extension};
pub use path::{is_same_or_inside, relative_path, resolve_inside};
pub use text_search::{FileMatch, contains_ignore_case, find_in_text};
pub use tree::{
    CollectionNode, DEFAULT_COLLECTION_EXCLUDES, MAX_SCAN_DEPTH, MAX_SCANNED_FILES,
    is_excluded_folder, is_markdown_file, sort_nodes,
};

text_enum! {
    /// Nature d'une collection.
    pub enum CollectionKind {
        /// Un dossier (et ses fichiers markdown, à toute profondeur).
        Folder => "folder",
        /// Un seul fichier markdown.
        File => "file",
    }
}

/// Une collection enregistrée ; son chemin (absolu, canonique) est sa clé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collection {
    /// Chemin absolu canonique.
    pub path: String,
    /// Dossier ou fichier.
    pub kind: CollectionKind,
    /// Nom affiché (le nom du dossier ou du fichier par défaut).
    pub display_name: String,
    /// Position dans la sidebar.
    pub sort_order: i64,
    /// Date d'ouverture (format SQLite).
    pub created_at: String,
}

/// Un fichier à ouvrir dans un onglet : sa collection et son chemin relatif à elle (le nom
/// du fichier pour une collection fichier).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionFileRef {
    /// Chemin de la collection.
    pub root: String,
    /// Chemin relatif, séparateurs `/`.
    pub path: String,
}
