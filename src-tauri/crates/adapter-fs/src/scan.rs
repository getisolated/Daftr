//! Parcours d'un dossier de collection.

use std::path::Path;

use domain::collection::{
    CollectionKind, CollectionNode, MAX_SCAN_DEPTH, MAX_SCANNED_FILES, is_excluded_folder,
    is_markdown_file, sort_nodes,
};

/// Résultat d'un parcours.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanResult {
    /// Arbre markdown.
    pub tree: Vec<CollectionNode>,
    /// Chemins relatifs de tous les fichiers markdown (ce que parcourt la recherche).
    pub files: Vec<String>,
    /// Le parcours s'est arrêté à `MAX_SCANNED_FILES` ou `MAX_SCAN_DEPTH`.
    pub truncated: bool,
}

/// Arbre markdown d'un dossier : seulement les `.md`, et seulement les dossiers qui en
/// contiennent (à toute profondeur). Les dossiers illisibles sont sautés.
pub fn scan_folder(root: &Path, excludes: &[String]) -> ScanResult {
    let mut scan = Scan {
        excludes,
        result: ScanResult::default(),
    };
    let tree = scan.visit(root, "", 0);

    ScanResult {
        tree,
        ..scan.result
    }
}

struct Scan<'a> {
    excludes: &'a [String],
    result: ScanResult,
}

impl Scan<'_> {
    fn visit(&mut self, directory: &Path, relative: &str, depth: usize) -> Vec<CollectionNode> {
        if depth > MAX_SCAN_DEPTH {
            self.result.truncated = true;
            return Vec::new();
        }
        let Ok(entries) = std::fs::read_dir(directory) else {
            return Vec::new();
        };
        let mut nodes = Vec::new();

        for entry in entries.flatten() {
            if self.result.files.len() >= MAX_SCANNED_FILES {
                self.result.truncated = true;
                break;
            }
            nodes.extend(self.visit_entry(&entry, relative, depth));
        }
        sort_nodes(&mut nodes);
        nodes
    }

    fn visit_entry(
        &mut self,
        entry: &std::fs::DirEntry,
        relative: &str,
        depth: usize,
    ) -> Option<CollectionNode> {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = if relative.is_empty() {
            name.clone()
        } else {
            format!("{relative}/{name}")
        };
        let file_type = entry.file_type().ok()?;

        if file_type.is_dir() {
            if is_excluded_folder(&name, self.excludes) {
                return None;
            }
            let children = self.visit(&entry.path(), &path, depth + 1);
            // Un dossier sans markdown nulle part dessous n'est pas montré
            return (!children.is_empty()).then_some(CollectionNode {
                name,
                path,
                kind: CollectionKind::Folder,
                children,
            });
        }
        if file_type.is_file() && is_markdown_file(&name) {
            self.result.files.push(path.clone());
            return Some(CollectionNode {
                name,
                path,
                kind: CollectionKind::File,
                children: Vec::new(),
            });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::collection::DEFAULT_COLLECTION_EXCLUDES;

    fn fixture() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        let files = [
            "README.md",
            "docs/guide.md",
            "docs/deep/more.MD",
            "docs/image.png",
            "src/index.ts",
            "node_modules/pkg/readme.md",
            ".github/notes.md",
            "empty/nothing.txt",
        ];
        for file in files {
            let path = root.path().join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "").unwrap();
        }
        root
    }

    fn excludes() -> Vec<String> {
        DEFAULT_COLLECTION_EXCLUDES.map(str::to_owned).to_vec()
    }

    #[test]
    fn keeps_only_markdown_files_and_the_folders_holding_some() {
        let root = fixture();

        let mut scan = scan_folder(root.path(), &excludes());
        scan.files.sort();

        assert!(!scan.truncated);
        assert_eq!(
            scan.files,
            ["README.md", "docs/deep/more.MD", "docs/guide.md"]
        );
        let names: Vec<&str> = scan.tree.iter().map(|node| node.name.as_str()).collect();
        assert_eq!(names, ["docs", "README.md"]);
        let docs: Vec<&str> = scan.tree[0]
            .children
            .iter()
            .map(|node| node.path.as_str())
            .collect();
        assert_eq!(docs, ["docs/deep", "docs/guide.md"]);
    }

    #[test]
    fn skips_excluded_and_hidden_folders() {
        let root = fixture();
        let scan = scan_folder(root.path(), &excludes());

        assert!(
            scan.files
                .iter()
                .all(|file| !file.starts_with("node_modules") && !file.starts_with(".github"))
        );
    }

    #[test]
    fn a_missing_folder_has_an_empty_tree() {
        let scan = scan_folder(Path::new("does/not/exist"), &[]);
        assert_eq!(scan, ScanResult::default());
    }
}
