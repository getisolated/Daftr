//! Événements des collections, publiés en arrière-plan vers la fenêtre principale.

use domain::collection::{CollectionGraph, CollectionNode, FileMatch};

/// Événement d'une collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CollectionEvent {
    /// Arbre markdown (nouveau ou changé) d'une collection.
    Tree {
        /// Collection.
        root: String,
        /// Arbre.
        tree: Vec<CollectionNode>,
        /// Le parcours s'est arrêté à une limite : l'arbre est incomplet.
        truncated: bool,
    },
    /// La collection n'a pas pu être lue (dossier déplacé, droits…).
    Error {
        /// Collection.
        root: String,
        /// Message du système.
        message: String,
    },
    /// Fichiers markdown modifiés sur le disque : les éditeurs ouverts peuvent les recharger.
    Changed {
        /// Collection.
        root: String,
        /// Chemins relatifs.
        paths: Vec<String>,
    },
    /// Résultats d'une recherche, par fichier ; le dernier a `done` à `true` et aucun fichier.
    SearchResults {
        /// Identifiant de la recherche.
        request_id: u64,
        /// Collection du fichier (vide pour le dernier).
        root: String,
        /// Chemin relatif du fichier (vide pour le dernier).
        path: String,
        /// Lignes trouvées (aucune si seul le nom correspond).
        matches: Vec<FileMatch>,
        /// Fin de la recherche.
        done: bool,
    },
    /// Graphe des liens d'une collection.
    Graph {
        /// Collection.
        root: String,
        /// Graphe.
        graph: CollectionGraph,
    },
}
