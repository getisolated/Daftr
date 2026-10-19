//! Arbre markdown d'une collection : quels fichiers et dossiers y figurent, dans quel ordre.

use std::cmp::Ordering;

use super::CollectionKind;

/// Dossiers jamais utiles dans un arbre de documentation (sorties de build, dépendances,
/// caches d'outils).
pub const DEFAULT_COLLECTION_EXCLUDES: [&str; 19] = [
    "node_modules",
    "bin",
    "obj",
    "build",
    "dist",
    "out",
    "target",
    "coverage",
    "vendor",
    "__pycache__",
    "venv",
    ".venv",
    ".git",
    ".angular",
    ".next",
    ".nuxt",
    ".vs",
    ".vscode",
    ".idea",
];

/// Au-delà, le parcours s'arrête : un disque entier ouvert par erreur ne doit rien bloquer.
pub const MAX_SCANNED_FILES: usize = 20_000;

/// Profondeur maximale parcourue.
pub const MAX_SCAN_DEPTH: usize = 32;

/// Une entrée de l'arbre ; `path` est relatif à la racine, séparateurs `/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionNode {
    /// Nom du fichier ou du dossier.
    pub name: String,
    /// Chemin relatif.
    pub path: String,
    /// Dossier ou fichier.
    pub kind: CollectionKind,
    /// Contenu d'un dossier (vide pour un fichier).
    pub children: Vec<CollectionNode>,
}

/// Fichier markdown (`.md`, sans tenir compte de la casse).
pub fn is_markdown_file(name: &str) -> bool {
    name.len() >= 3
        && name
            .get(name.len() - 3..)
            .is_some_and(|ext| ext.eq_ignore_ascii_case(".md"))
}

/// Les dossiers cachés (`.quelquechose`) et les noms exclus (sans tenir compte de la casse)
/// sont ignorés.
pub fn is_excluded_folder(name: &str, excludes: &[String]) -> bool {
    let lower = name.to_lowercase();
    name.starts_with('.')
        || excludes
            .iter()
            .any(|exclude| exclude.to_lowercase() == lower)
}

/// Trie un niveau de l'arbre : dossiers d'abord, puis ordre naturel insensible à la casse
/// (`note 2` avant `note 10`).
pub fn sort_nodes(nodes: &mut [CollectionNode]) {
    nodes.sort_by(compare_nodes);
}

fn compare_nodes(a: &CollectionNode, b: &CollectionNode) -> Ordering {
    let folder_first = |node: &CollectionNode| node.kind != CollectionKind::Folder;
    folder_first(a)
        .cmp(&folder_first(b))
        .then_with(|| natural_cmp(&a.name, &b.name))
}

/// Ordre naturel : les suites de chiffres se comparent par valeur, le reste sans casse.
fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (chunks_a, chunks_b) = (chunks(a), chunks(b));

    for (left, right) in chunks_a.iter().zip(&chunks_b) {
        let order = compare_chunks(left, right);
        if order != Ordering::Equal {
            return order;
        }
    }
    chunks_a.len().cmp(&chunks_b.len())
}

fn chunks(text: &str) -> Vec<String> {
    let mut chunks: Vec<String> = Vec::new();

    for c in text.chars().flat_map(char::to_lowercase) {
        match chunks.last_mut() {
            Some(last) if same_class(last, c) => last.push(c),
            Some(_) | None => chunks.push(c.to_string()),
        }
    }
    chunks
}

fn same_class(chunk: &str, c: char) -> bool {
    chunk
        .chars()
        .next()
        .is_some_and(|first| first.is_ascii_digit() == c.is_ascii_digit())
}

fn compare_chunks(left: &str, right: &str) -> Ordering {
    let both_numbers = left.starts_with(|c: char| c.is_ascii_digit())
        && right.starts_with(|c: char| c.is_ascii_digit());

    if !both_numbers {
        return left.cmp(right);
    }
    let (left, right) = (left.trim_start_matches('0'), right.trim_start_matches('0'));
    left.len().cmp(&right.len()).then_with(|| left.cmp(right))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(name: &str, kind: CollectionKind) -> CollectionNode {
        CollectionNode {
            name: name.to_owned(),
            path: name.to_owned(),
            kind,
            children: Vec::new(),
        }
    }

    fn excludes(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_owned()).collect()
    }

    #[test]
    fn recognizes_markdown_files_whatever_the_case() {
        assert!(is_markdown_file("README.md"));
        assert!(is_markdown_file("more.MD"));
        assert!(!is_markdown_file("image.png"));
        assert!(!is_markdown_file("md"));
        assert!(!is_markdown_file("été"));
    }

    #[test]
    fn excludes_listed_and_hidden_folders() {
        assert!(is_excluded_folder(
            "Node_Modules",
            &excludes(&["node_modules"])
        ));
        assert!(is_excluded_folder(".cache", &[]));
        assert!(!is_excluded_folder("docs", &excludes(&["node_modules"])));
    }

    #[test]
    fn sorts_folders_first_then_naturally() {
        let mut nodes = vec![
            node("note 10.md", CollectionKind::File),
            node("README.md", CollectionKind::File),
            node("docs", CollectionKind::Folder),
            node("note 2.md", CollectionKind::File),
            node("Alpha", CollectionKind::Folder),
        ];

        sort_nodes(&mut nodes);

        let names: Vec<&str> = nodes.iter().map(|node| node.name.as_str()).collect();
        assert_eq!(
            names,
            ["Alpha", "docs", "note 2.md", "note 10.md", "README.md"]
        );
    }
}
