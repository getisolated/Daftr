//! État et boucle du thread de surveillance.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use app::collections::CollectionEvent;
use app::ports::CollectionEventSink;
use domain::collection::{CollectionKind, CollectionNode, is_markdown_file, relative_path};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use super::Command;
use super::search::{self, Snapshot};
use crate::scan::scan_folder;

/// Les rafales d'événements (un checkout git, un enregistrement) sont traitées une fois.
const RESCAN_DELAY: Duration = Duration::from_millis(250);

/// Une collection surveillée.
struct Watched {
    root: String,
    kind: CollectionKind,
    files: Arc<Vec<String>>,
    /// Dernier arbre envoyé ; `None` force l'envoi au prochain parcours.
    sent_tree: Option<Vec<CollectionNode>>,
    /// Gardé en vie tant que la collection est surveillée.
    _watcher: Option<RecommendedWatcher>,
    deadline: Option<Instant>,
    changed: BTreeSet<String>,
}

pub(super) struct Worker {
    sink: Arc<dyn CollectionEventSink>,
    commands: Sender<Command>,
    excludes: Vec<String>,
    watched: Vec<Watched>,
    current_search: Arc<AtomicU64>,
}

impl Worker {
    pub(super) fn new(
        sink: Arc<dyn CollectionEventSink>,
        commands: Sender<Command>,
        excludes: Vec<String>,
        current_search: Arc<AtomicU64>,
    ) -> Self {
        Self {
            sink,
            commands,
            excludes,
            watched: Vec::new(),
            current_search,
        }
    }

    /// Traite les demandes jusqu'à `Stop`, en reparcourant les collections dont le délai
    /// après un changement est écoulé.
    pub(super) fn run(mut self, receiver: &Receiver<Command>) {
        loop {
            let received = match self.next_deadline() {
                Some(deadline) => {
                    receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()))
                }
                None => receiver.recv().map_err(|_| RecvTimeoutError::Disconnected),
            };
            match received {
                Ok(Command::Stop) | Err(RecvTimeoutError::Disconnected) => break,
                Ok(command) => self.handle(command),
                Err(RecvTimeoutError::Timeout) => {}
            }
            self.flush_due();
        }
    }

    fn handle(&mut self, command: Command) {
        match command {
            Command::Watch { root, kind } => self.watch(root, kind),
            Command::Unwatch { root } => self.watched.retain(|entry| entry.root != root),
            Command::SetExcludes(excludes) => self.set_excludes(excludes),
            Command::Search { request_id, query } => self.search(request_id, query),
            Command::Graph { root } => self.graph(&root),
            Command::FileEvent { root, paths } => self.note_changes(&root, &paths),
            Command::Stop => {}
        }
    }

    fn watch(&mut self, root: String, kind: CollectionKind) {
        self.watched.retain(|entry| entry.root != root);
        let watcher = self.start_watcher(&root, kind);
        let mut entry = Watched {
            root,
            kind,
            files: Arc::new(Vec::new()),
            sent_tree: None,
            _watcher: watcher,
            deadline: None,
            changed: BTreeSet::new(),
        };

        rescan(&mut entry, &self.excludes, self.sink.as_ref());
        self.watched.push(entry);
    }

    /// Surveillance du système (récursive pour un dossier) ; un chemin absent n'est pas
    /// surveillé : le parcours signale l'erreur, la collection l'affiche.
    fn start_watcher(&self, root: &str, kind: CollectionKind) -> Option<RecommendedWatcher> {
        let commands = self.commands.clone();
        let owner = root.to_owned();
        let handler = move |event: notify::Result<notify::Event>| {
            let paths = event.map(|event| event.paths).unwrap_or_default();
            // Le thread s'arrête avant ses surveillances : un envoi perdu n'a plus d'objet
            drop(commands.send(Command::FileEvent {
                root: owner.clone(),
                paths,
            }));
        };
        let mode = match kind {
            CollectionKind::Folder => RecursiveMode::Recursive,
            CollectionKind::File => RecursiveMode::NonRecursive,
        };

        let mut watcher = notify::recommended_watcher(handler).ok()?;
        watcher.watch(Path::new(root), mode).ok()?;
        Some(watcher)
    }

    fn set_excludes(&mut self, excludes: Vec<String>) {
        self.excludes = excludes;

        for entry in &mut self.watched {
            entry.sent_tree = None;
            rescan(entry, &self.excludes, self.sink.as_ref());
        }
    }

    fn note_changes(&mut self, root: &str, paths: &[PathBuf]) {
        let Some(entry) = self.watched.iter_mut().find(|entry| entry.root == root) else {
            return;
        };
        let changed: Vec<String> = paths
            .iter()
            .filter_map(|path| changed_markdown(entry, path))
            .collect();
        entry.changed.extend(changed);
        entry.deadline = Some(Instant::now() + RESCAN_DELAY);
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.watched.iter().filter_map(|entry| entry.deadline).min()
    }

    fn flush_due(&mut self) {
        let now = Instant::now();

        for entry in &mut self.watched {
            if entry.deadline.is_some_and(|deadline| deadline <= now) {
                entry.deadline = None;
                rescan(entry, &self.excludes, self.sink.as_ref());
                publish_changed(entry, self.sink.as_ref());
            }
        }
    }

    fn search(&self, request_id: u64, query: String) {
        self.current_search.store(request_id, Ordering::Relaxed);
        let snapshots = self.watched.iter().map(snapshot).collect();

        search::spawn_search(search::SearchJob {
            request_id,
            query,
            snapshots,
            current: Arc::clone(&self.current_search),
            sink: Arc::clone(&self.sink),
        });
    }

    fn graph(&self, root: &str) {
        if let Some(entry) = self.watched.iter().find(|entry| entry.root == root) {
            search::spawn_graph(snapshot(entry), Arc::clone(&self.sink));
        }
    }
}

/// Photographie des fichiers d'une collection, lue par un autre thread ; la liste est
/// partagée (`Arc`), pas copiée.
fn snapshot(entry: &Watched) -> Snapshot {
    Snapshot {
        root: entry.root.clone(),
        kind: entry.kind,
        files: Arc::clone(&entry.files),
    }
}

/// Chemin relatif d'un fichier markdown changé ; les autres fichiers sont ignorés.
fn changed_markdown(entry: &Watched, path: &Path) -> Option<String> {
    let relative = match entry.kind {
        CollectionKind::File => path.file_name()?.to_string_lossy().into_owned(),
        CollectionKind::Folder => relative_path(Path::new(&entry.root), path)?,
    };
    is_markdown_file(&relative).then_some(relative)
}

fn publish_changed(entry: &mut Watched, sink: &dyn CollectionEventSink) {
    if entry.changed.is_empty() {
        return;
    }
    let paths = std::mem::take(&mut entry.changed).into_iter().collect();
    sink.publish(CollectionEvent::Changed {
        root: entry.root.clone(),
        paths,
    });
}

/// Reparcourt la collection et envoie son arbre s'il a changé (ou une erreur).
fn rescan(entry: &mut Watched, excludes: &[String], sink: &dyn CollectionEventSink) {
    match read_tree(entry, excludes) {
        Ok((tree, files, truncated)) => {
            entry.files = Arc::new(files);
            if entry.sent_tree.as_ref() != Some(&tree) {
                entry.sent_tree = Some(tree.clone());
                sink.publish(CollectionEvent::Tree {
                    root: entry.root.clone(),
                    tree,
                    truncated,
                });
            }
        }
        Err(error) => {
            entry.sent_tree = None;
            sink.publish(CollectionEvent::Error {
                root: entry.root.clone(),
                message: error.to_string(),
            });
        }
    }
}

type Tree = (Vec<CollectionNode>, Vec<String>, bool);

fn read_tree(entry: &Watched, excludes: &[String]) -> std::io::Result<Tree> {
    let root = Path::new(&entry.root);
    let metadata = std::fs::metadata(root)?;

    match entry.kind {
        CollectionKind::File => {
            let name = root
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let node = CollectionNode {
                name: name.clone(),
                path: name.clone(),
                kind: CollectionKind::File,
                children: Vec::new(),
            };
            Ok((vec![node], vec![name], false))
        }
        CollectionKind::Folder if metadata.is_dir() => {
            let scan = scan_folder(root, excludes);
            Ok((scan.tree, scan.files, scan.truncated))
        }
        CollectionKind::Folder => Err(std::io::Error::new(
            std::io::ErrorKind::NotADirectory,
            "Not a folder",
        )),
    }
}
