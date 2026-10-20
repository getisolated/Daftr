//! Recherche et graphe, chacun sur un thread éphémère : la surveillance continue pendant
//! qu'ils lisent les fichiers.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use app::collections::CollectionEvent;
use app::ports::CollectionEventSink;
use domain::collection::{
    CollectionKind, FileContent, build_collection_graph, contains_ignore_case, find_in_text,
};

const MAX_MATCHES_PER_FILE: usize = 5;
const MAX_SEARCH_RESULTS: usize = 300;
/// Au-delà, un graphe serait illisible (et long à construire) : seuls les premiers fichiers
/// sont dessinés.
const MAX_GRAPH_FILES: usize = 2000;

/// Fichiers d'une collection au moment de la demande.
pub(super) struct Snapshot {
    pub(super) root: String,
    pub(super) kind: CollectionKind,
    pub(super) files: Arc<Vec<String>>,
}

impl Snapshot {
    fn absolute(&self, file: &str) -> PathBuf {
        match self.kind {
            CollectionKind::File => PathBuf::from(&self.root),
            CollectionKind::Folder => Path::new(&self.root).join(file),
        }
    }
}

/// Une recherche à mener.
pub(super) struct SearchJob {
    pub(super) request_id: u64,
    pub(super) query: String,
    pub(super) snapshots: Vec<Snapshot>,
    /// Recherche en cours : une valeur différente arrête celle-ci.
    pub(super) current: Arc<AtomicU64>,
    pub(super) sink: Arc<dyn CollectionEventSink>,
}

pub(super) fn spawn_search(job: SearchJob) {
    spawn("collections-search", move || run_search(&job));
}

pub(super) fn spawn_graph(snapshot: Snapshot, sink: Arc<dyn CollectionEventSink>) {
    spawn("collections-graph", move || {
        let graph = build_collection_graph(&read_files(&snapshot));
        sink.publish(CollectionEvent::Graph {
            root: snapshot.root,
            graph,
        });
    });
}

fn spawn(name: &str, job: impl FnOnce() + Send + 'static) {
    if let Err(error) = std::thread::Builder::new().name(name.to_owned()).spawn(job) {
        tracing::error!(%error, name, "thread non démarré");
    }
}

/// Lit les fichiers markdown de chaque collection ; les lignes trouvées arrivent par lots
/// (un par fichier), puis un dernier événement `done`.
fn run_search(job: &SearchJob) {
    let mut total = 0;

    for snapshot in &job.snapshots {
        for file in snapshot.files.iter() {
            if !is_current(job) || total >= MAX_SEARCH_RESULTS {
                break;
            }
            total += search_file(job, snapshot, file);
        }
    }
    if is_current(job) {
        job.sink.publish(results(
            job.request_id,
            String::new(),
            String::new(),
            Vec::new(),
            true,
        ));
    }
}

/// Cherche dans un fichier ; rend le nombre de résultats comptés (au moins 1 si seul le nom
/// correspond, 0 si rien).
fn search_file(job: &SearchJob, snapshot: &Snapshot, file: &str) -> usize {
    let Ok(content) = std::fs::read_to_string(snapshot.absolute(file)) else {
        return 0;
    };
    let matches = find_in_text(&content, &job.query, MAX_MATCHES_PER_FILE);

    if matches.is_empty() && !contains_ignore_case(file, &job.query) {
        return 0;
    }
    let found = matches.len().max(1);
    job.sink.publish(results(
        job.request_id,
        snapshot.root.clone(),
        file.to_owned(),
        matches,
        false,
    ));
    found
}

fn results(
    request_id: u64,
    root: String,
    path: String,
    matches: Vec<domain::collection::FileMatch>,
    done: bool,
) -> CollectionEvent {
    CollectionEvent::SearchResults {
        request_id,
        root,
        path,
        matches,
        done,
    }
}

fn is_current(job: &SearchJob) -> bool {
    job.current.load(Ordering::Relaxed) == job.request_id
}

/// Les fichiers à relier ; un fichier illisible compte comme vide.
fn read_files(snapshot: &Snapshot) -> Vec<FileContent> {
    snapshot
        .files
        .iter()
        .take(MAX_GRAPH_FILES)
        .map(|file| FileContent {
            path: file.clone(),
            content: std::fs::read_to_string(snapshot.absolute(file)).unwrap_or_default(),
        })
        .collect()
}
