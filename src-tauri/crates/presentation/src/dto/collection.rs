//! Collections.

use app::collections::CollectionEvent;
use domain::collection::{
    Collection, CollectionFileRef, CollectionGraph, CollectionNode, FileMatch, GraphLink, GraphNode,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Une collection (ligne de la table `collections`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct CollectionDto {
    /// Chemin absolu, sa clé.
    pub path: String,
    /// `folder` ou `file`.
    #[ts(type = "'folder' | 'file'")]
    pub kind: &'static str,
    /// Nom affiché.
    pub display_name: String,
    /// Position.
    #[ts(type = "number")]
    pub sort_order: i64,
    /// Date d'ouverture.
    pub created_at: String,
}

impl From<Collection> for CollectionDto {
    fn from(collection: Collection) -> Self {
        Self {
            path: collection.path,
            kind: collection.kind.as_str(),
            display_name: collection.display_name,
            sort_order: collection.sort_order,
            created_at: collection.created_at,
        }
    }
}

/// Une entrée de l'arbre d'une collection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct CollectionNodeDto {
    /// Nom.
    pub name: String,
    /// Chemin relatif.
    pub path: String,
    /// `folder` ou `file`.
    #[ts(type = "'folder' | 'file'")]
    pub kind: &'static str,
    /// Contenu d'un dossier (absent pour un fichier).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub children: Option<Vec<CollectionNodeDto>>,
}

impl From<CollectionNode> for CollectionNodeDto {
    fn from(node: CollectionNode) -> Self {
        let is_folder = node.kind == domain::collection::CollectionKind::Folder;
        Self {
            name: node.name,
            path: node.path,
            kind: node.kind.as_str(),
            children: is_folder.then(|| node.children.into_iter().map(Self::from).collect()),
        }
    }
}

/// Un fichier d'une collection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct CollectionFileRefDto {
    /// Collection.
    pub root: String,
    /// Chemin relatif.
    pub path: String,
}

impl From<CollectionFileRef> for CollectionFileRefDto {
    fn from(file: CollectionFileRef) -> Self {
        Self {
            root: file.root,
            path: file.path,
        }
    }
}

/// Une ligne trouvée par une recherche (positions en unités UTF-16).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct FileMatchDto {
    /// Numéro de ligne, à partir de 1.
    #[ts(type = "number")]
    pub line: usize,
    /// La ligne, coupée autour de la correspondance.
    pub text: String,
    /// Début de la correspondance.
    #[ts(type = "number")]
    pub from: usize,
    /// Fin de la correspondance.
    #[ts(type = "number")]
    pub to: usize,
}

impl From<FileMatch> for FileMatchDto {
    fn from(found: FileMatch) -> Self {
        Self {
            line: found.line,
            text: found.text,
            from: found.from,
            to: found.to,
        }
    }
}

/// Un nœud du graphe d'une collection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct GraphNodeDto {
    /// Chemin relatif.
    pub path: String,
    /// Nombre de liens.
    #[ts(type = "number")]
    pub degree: usize,
    /// Cible d'un lien vers un fichier absent.
    pub missing: bool,
}

/// Un lien du graphe d'une collection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct GraphLinkDto {
    /// Fichier contenant le lien.
    pub source: String,
    /// Fichier visé.
    pub target: String,
}

/// Graphe des liens d'une collection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct CollectionGraphDto {
    /// Nœuds.
    pub nodes: Vec<GraphNodeDto>,
    /// Liens.
    pub links: Vec<GraphLinkDto>,
}

impl From<CollectionGraph> for CollectionGraphDto {
    fn from(graph: CollectionGraph) -> Self {
        Self {
            nodes: graph.nodes.into_iter().map(node_dto).collect(),
            links: graph.links.into_iter().map(link_dto).collect(),
        }
    }
}

fn node_dto(node: GraphNode) -> GraphNodeDto {
    GraphNodeDto {
        path: node.path,
        degree: node.degree,
        missing: node.missing,
    }
}

fn link_dto(link: GraphLink) -> GraphLinkDto {
    GraphLinkDto {
        source: link.source,
        target: link.target,
    }
}

/// Événement des collections (`collections:event`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum CollectionEventDto {
    /// Arbre d'une collection.
    Tree {
        /// Collection.
        root: String,
        /// Arbre.
        tree: Vec<CollectionNodeDto>,
        /// Arbre incomplet.
        truncated: bool,
    },
    /// Collection illisible.
    Error {
        /// Collection.
        root: String,
        /// Message du système.
        message: String,
    },
    /// Fichiers modifiés sur le disque.
    Changed {
        /// Collection.
        root: String,
        /// Chemins relatifs.
        paths: Vec<String>,
    },
    /// Résultats d'une recherche, par fichier.
    SearchResults {
        /// Recherche.
        #[ts(type = "number")]
        request_id: u64,
        /// Collection du fichier.
        root: String,
        /// Fichier.
        path: String,
        /// Lignes trouvées.
        matches: Vec<FileMatchDto>,
        /// Fin de la recherche.
        done: bool,
    },
    /// Graphe d'une collection.
    Graph {
        /// Collection.
        root: String,
        /// Graphe.
        graph: CollectionGraphDto,
    },
}

impl From<CollectionEvent> for CollectionEventDto {
    fn from(event: CollectionEvent) -> Self {
        match event {
            CollectionEvent::Tree {
                root,
                tree,
                truncated,
            } => {
                let tree = tree.into_iter().map(CollectionNodeDto::from).collect();
                Self::Tree {
                    root,
                    tree,
                    truncated,
                }
            }
            CollectionEvent::Error { root, message } => Self::Error { root, message },
            CollectionEvent::Changed { root, paths } => Self::Changed { root, paths },
            CollectionEvent::SearchResults {
                request_id,
                root,
                path,
                matches,
                done,
            } => {
                let matches = matches.into_iter().map(FileMatchDto::from).collect();
                Self::SearchResults {
                    request_id,
                    root,
                    path,
                    matches,
                    done,
                }
            }
            CollectionEvent::Graph { root, graph } => Self::Graph {
                root,
                graph: graph.into(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serializes_events_like_the_electron_worker() {
        let event = CollectionEventDto::from(CollectionEvent::SearchResults {
            request_id: 3,
            root: "r".into(),
            path: "a.md".into(),
            matches: vec![],
            done: false,
        });

        let expected = json!({
            "type": "searchResults", "requestId": 3, "root": "r", "path": "a.md", "matches": [], "done": false,
        });
        assert_eq!(serde_json::to_value(event).unwrap(), expected);
    }

    #[test]
    fn files_have_no_children_field() {
        let file = CollectionNode {
            name: "a.md".into(),
            path: "a.md".into(),
            kind: domain::collection::CollectionKind::File,
            children: vec![],
        };
        let folder = CollectionNode {
            kind: domain::collection::CollectionKind::Folder,
            ..file.clone()
        };

        assert_eq!(
            serde_json::to_value(CollectionNodeDto::from(file))
                .unwrap()
                .get("children"),
            None
        );
        assert_eq!(
            serde_json::to_value(CollectionNodeDto::from(folder)).unwrap()["children"],
            json!([])
        );
    }
}
