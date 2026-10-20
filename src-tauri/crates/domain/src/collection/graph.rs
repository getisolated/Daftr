//! Graphe des liens entre les fichiers d'une collection (jamais entre collections).

use std::collections::{HashMap, HashSet};

use super::{extract_link_targets, is_folder_link, resolve_collection_link};

/// Un fichier lu : son chemin relatif et son texte.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileContent {
    /// Chemin relatif, séparateurs `/`.
    pub path: String,
    /// Texte markdown.
    pub content: String,
}

/// Un nœud du graphe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphNode {
    /// Chemin relatif.
    pub path: String,
    /// Nombre de liens entrants et sortants.
    pub degree: usize,
    /// Cible d'un lien dont le fichier n'existe pas.
    pub missing: bool,
}

/// Un lien orienté.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphLink {
    /// Fichier contenant le lien.
    pub source: String,
    /// Fichier visé.
    pub target: String,
}

/// Graphe d'une collection.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CollectionGraph {
    /// Les fichiers dans l'ordre donné, puis les cibles manquantes dans l'ordre de découverte.
    pub nodes: Vec<GraphNode>,
    /// Les liens, chacun une seule fois.
    pub links: Vec<GraphLink>,
}

/// Graphe des liens entre `files` : un lien vers un fichier connu prend la casse de ce
/// fichier ; les doublons et les liens vers soi-même sont ignorés.
pub fn build_collection_graph(files: &[FileContent]) -> CollectionGraph {
    let mut builder = GraphBuilder::new(files);

    for file in files {
        for target in extract_link_targets(&file.content) {
            let Some(resolved) = resolve_collection_link(&file.path, &target) else {
                continue;
            };
            if !is_folder_link(&target) {
                builder.add_link(&file.path, &resolved);
            } else if let Some(page) = builder.folder_page(&resolved) {
                builder.add_link(&file.path, &page);
            }
        }
    }
    builder.finish()
}

struct GraphBuilder {
    existing: HashMap<String, String>,
    nodes: Vec<GraphNode>,
    index: HashMap<String, usize>,
    seen: HashSet<(String, String)>,
    links: Vec<GraphLink>,
}

impl GraphBuilder {
    fn new(files: &[FileContent]) -> Self {
        let mut builder = Self {
            existing: files
                .iter()
                .map(|f| (f.path.to_lowercase(), f.path.clone()))
                .collect(),
            nodes: Vec::with_capacity(files.len()),
            index: HashMap::with_capacity(files.len()),
            seen: HashSet::new(),
            links: Vec::new(),
        };
        for file in files {
            builder.node_index(&file.path, false);
        }
        builder
    }

    /// Page qu'ouvre un lien vers un dossier (`resolved` = `<dossier>/index.md`) : son index.md,
    /// sinon son README.md ; aucune si le dossier existe sans l'une ni l'autre (le lien n'est pas
    /// cassé, il n'a juste pas de page à relier), l'index.md manquant si le dossier n'existe pas.
    fn folder_page(&self, resolved: &str) -> Option<String> {
        let folder = resolved.strip_suffix("index.md").unwrap_or(resolved);
        let lower = folder.to_lowercase();
        let page = |name: &str| self.existing.get(&format!("{lower}{name}")).cloned();

        if let Some(found) = page("index.md").or_else(|| page("readme.md")) {
            return Some(found);
        }
        let folder_exists = self.existing.keys().any(|path| path.starts_with(&lower));
        (!folder_exists).then(|| resolved.to_owned())
    }

    fn add_link(&mut self, source: &str, resolved: &str) {
        let known = self.existing.get(&resolved.to_lowercase()).cloned();
        let missing = known.is_none();
        let target = known.unwrap_or_else(|| resolved.to_owned());

        // La paire est gardée en clé : les deux chaînes sont copiées une fois par lien
        if target == source || !self.seen.insert((source.to_owned(), target.clone())) {
            return;
        }
        let source_index = self.node_index(source, false);
        let target_index = self.node_index(&target, missing);

        self.nodes[source_index].degree += 1;
        self.nodes[target_index].degree += 1;
        self.links.push(GraphLink {
            source: source.to_owned(),
            target,
        });
    }

    fn node_index(&mut self, path: &str, missing: bool) -> usize {
        if let Some(index) = self.index.get(path) {
            return *index;
        }
        self.nodes.push(GraphNode {
            path: path.to_owned(),
            degree: 0,
            missing,
        });
        self.index.insert(path.to_owned(), self.nodes.len() - 1);
        self.nodes.len() - 1
    }

    fn finish(self) -> CollectionGraph {
        CollectionGraph {
            nodes: self.nodes,
            links: self.links,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str, content: &str) -> FileContent {
        FileContent {
            path: path.to_owned(),
            content: content.to_owned(),
        }
    }

    fn link(source: &str, target: &str) -> GraphLink {
        GraphLink {
            source: source.to_owned(),
            target: target.to_owned(),
        }
    }

    fn node(path: &str, degree: usize, missing: bool) -> GraphNode {
        GraphNode {
            path: path.to_owned(),
            degree,
            missing,
        }
    }

    #[test]
    fn links_files_counts_degrees_and_flags_missing_targets() {
        let readme =
            "[guide](docs/Guide.md) [again](docs/guide.md) [gone](docs/gone.md) [web](https://a.b)";
        let graph = build_collection_graph(&[
            file("README.md", readme),
            file("docs/guide.md", "[home](../README.md) [self](./guide.md)"),
            file("docs/alone.md", ""),
        ]);

        assert_eq!(
            graph.links,
            [
                link("README.md", "docs/guide.md"),
                link("README.md", "docs/gone.md"),
                link("docs/guide.md", "README.md"),
            ]
        );
        assert_eq!(
            graph.nodes,
            [
                node("README.md", 3, false),
                node("docs/guide.md", 2, false),
                node("docs/alone.md", 0, false),
                node("docs/gone.md", 1, true),
            ]
        );
    }

    #[test]
    fn a_folder_link_leads_to_its_index_or_readme_page() {
        let graph = build_collection_graph(&[
            file(
                "a/v26/client.md",
                "[root](../) [guide](../../guide/) [gone](../../nothing/)",
            ),
            file("guide/README.md", ""),
        ]);

        assert_eq!(
            graph.links,
            [
                link("a/v26/client.md", "guide/README.md"),
                link("a/v26/client.md", "nothing/index.md"),
            ]
        );
    }

    #[test]
    fn an_empty_collection_has_an_empty_graph() {
        assert_eq!(build_collection_graph(&[]), CollectionGraph::default());
    }
}
