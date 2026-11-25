//! Surveillance des dossiers des collections partagées : un changement de fichier `.md` est
//! signalé une fois par rafale (un enregistrement, un checkout git), après un court délai.
//!
//! Indépendante de la surveillance de l'interface : elle ne dépend ni du profil actif ni des
//! collections dépliées, et ne fait aucun parcours (c'est le moteur de synchronisation qui
//! relit les fichiers).

use std::collections::{BTreeSet, HashMap};
use std::ffi::OsStr;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use notify::event::{EventKind, ModifyKind};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

/// Délai après le dernier événement d'un dossier avant de le signaler.
const SETTLE: Duration = Duration::from_millis(400);

/// Préfixe des fichiers temporaires de l'écriture atomique de la synchronisation.
const TEMPORARY_PREFIX: &str = ".daftr-sync-";

/// Appelé avec le dossier surveillé dont des fichiers ont changé.
pub type OnChange = Arc<dyn Fn(&str) + Send + Sync>;

enum Command {
    Watch(String),
    Unwatch(String),
    Event(String),
    Stop,
}

/// Poignée du thread de surveillance.
pub struct ShareWatcher {
    commands: Sender<Command>,
}

impl ShareWatcher {
    /// Démarre le thread ; `on_change` est appelé depuis lui.
    pub fn start(on_change: OnChange) -> io::Result<Self> {
        let (commands, receiver) = mpsc::channel();
        let worker = Worker {
            on_change,
            commands: commands.clone(),
            watchers: HashMap::new(),
            due: HashMap::new(),
        };
        std::thread::Builder::new()
            .name("share-watcher".to_owned())
            .spawn(move || worker.run(&receiver))?;
        Ok(Self { commands })
    }

    /// Surveille `root` (déjà surveillé : surveillance renouvelée).
    pub fn watch(&self, root: &str) {
        self.send(Command::Watch(root.to_owned()));
    }

    /// Arrête de surveiller `root`.
    pub fn unwatch(&self, root: &str) {
        self.send(Command::Unwatch(root.to_owned()));
    }

    fn send(&self, command: Command) {
        if self.commands.send(command).is_err() {
            tracing::warn!("thread de surveillance des partages arrêté, demande ignorée");
        }
    }
}

impl Drop for ShareWatcher {
    fn drop(&mut self) {
        self.send(Command::Stop);
    }
}

struct Worker {
    on_change: OnChange,
    commands: Sender<Command>,
    watchers: HashMap<String, RecommendedWatcher>,
    /// Dossiers à signaler, avec l'heure du signalement.
    due: HashMap<String, Instant>,
}

impl Worker {
    fn run(mut self, receiver: &Receiver<Command>) {
        loop {
            let next = self.due.values().min().copied();
            let received = match next {
                Some(deadline) => {
                    receiver.recv_timeout(deadline.saturating_duration_since(Instant::now()))
                }
                None => receiver.recv().map_err(|_| RecvTimeoutError::Disconnected),
            };
            match received {
                Ok(Command::Stop) | Err(RecvTimeoutError::Disconnected) => return,
                Ok(Command::Watch(root)) => self.watch(root),
                Ok(Command::Unwatch(root)) => self.unwatch(&root),
                Ok(Command::Event(root)) => {
                    self.due.insert(root, Instant::now() + SETTLE);
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
            self.flush();
        }
    }

    fn watch(&mut self, root: String) {
        self.unwatch(&root);
        let commands = self.commands.clone();
        let owner = root.clone();
        // Le système peut citer la racine sous son nom canonique (`/var` devient `/private/var`)
        let roots: Vec<PathBuf> = std::iter::once(PathBuf::from(&root))
            .chain(std::fs::canonicalize(&root))
            .collect();
        let handler = move |event: notify::Result<notify::Event>| {
            let relevant = event.is_ok_and(|event| concerns_files(&event, &roots));
            if relevant {
                // Le thread s'arrête avant ses surveillances : un envoi perdu n'a plus d'objet
                drop(commands.send(Command::Event(owner.clone())));
            }
        };
        let started = notify::recommended_watcher(handler).and_then(|mut watcher| {
            watcher.watch(Path::new(&root), RecursiveMode::Recursive)?;
            Ok(watcher)
        });
        match started {
            Ok(watcher) => {
                self.watchers.insert(root, watcher);
            }
            // Un dossier absent n'est pas surveillé : le moteur le signale « introuvable »
            Err(error) => tracing::debug!(%error, %root, "dossier partagé non surveillé"),
        }
    }

    fn unwatch(&mut self, root: &str) {
        self.watchers.remove(root);
        self.due.remove(root);
    }

    fn flush(&mut self) {
        let now = Instant::now();
        let ready: BTreeSet<String> = self
            .due
            .iter()
            .filter(|(_, at)| **at <= now)
            .map(|(root, _)| root.clone())
            .collect();
        for root in ready {
            self.due.remove(&root);
            (self.on_change)(&root);
        }
    }
}

/// L'événement touche des fichiers à synchroniser. Le dossier surveillé lui-même ne compte
/// pas quand son contenu change (macOS le signale à chaque écriture, même d'un fichier ignoré),
/// sauf s'il est supprimé ou renommé : le moteur doit alors le déclarer introuvable.
fn concerns_files(event: &notify::Event, roots: &[PathBuf]) -> bool {
    let root_gone = matches!(
        event.kind,
        EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(_))
    );
    event.paths.iter().any(|path| {
        let is_root = roots.iter().any(|root| root == path);
        is_relevant(path) && (!is_root || root_gone)
    })
}

/// Un chemin dont le changement peut toucher la synchronisation : un `.md`, ou un dossier
/// (sans extension) ; ni un autre type de fichier, ni un temporaire de la synchronisation.
fn is_relevant(path: &Path) -> bool {
    let name = path.file_name().and_then(OsStr::to_str).unwrap_or_default();
    if name.starts_with(TEMPORARY_PREFIX) {
        return false;
    }
    match path.extension().and_then(OsStr::to_str) {
        Some(extension) => extension.eq_ignore_ascii_case("md"),
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use super::*;

    fn collecting() -> (OnChange, Arc<Mutex<Vec<String>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        let on_change: OnChange = Arc::new(move |root| {
            if let Ok(mut seen) = sink.lock() {
                seen.push(root.to_owned());
            }
        });
        (on_change, seen)
    }

    fn count(seen: &Arc<Mutex<Vec<String>>>) -> usize {
        seen.lock().map_or(0, |seen| seen.len())
    }

    fn wait_for(condition: impl Fn() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if condition() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        false
    }

    fn event(kind: EventKind, path: &str) -> notify::Event {
        notify::Event::new(kind).add_path(PathBuf::from(path))
    }

    #[test]
    fn the_watched_folder_itself_is_reported_only_when_it_disappears() {
        use notify::event::{CreateKind, MetadataKind, RemoveKind, RenameMode};

        let roots = [PathBuf::from("/data/notes")];
        let touched = event(
            EventKind::Modify(ModifyKind::Metadata(MetadataKind::Any)),
            "/data/notes",
        );
        let removed = event(EventKind::Remove(RemoveKind::Folder), "/data/notes");
        let renamed = event(
            EventKind::Modify(ModifyKind::Name(RenameMode::Any)),
            "/data/notes",
        );
        let file = event(EventKind::Create(CreateKind::File), "/data/notes/a.md");
        let sub_folder = event(EventKind::Create(CreateKind::Folder), "/data/notes/sub");
        let image = event(EventKind::Create(CreateKind::File), "/data/notes/a.png");

        assert!(!concerns_files(&touched, &roots));
        assert!(concerns_files(&removed, &roots));
        assert!(concerns_files(&renamed, &roots));
        assert!(concerns_files(&file, &roots));
        assert!(concerns_files(&sub_folder, &roots));
        assert!(!concerns_files(&image, &roots));
    }

    #[test]
    fn a_saved_markdown_file_is_reported_once_per_burst() {
        let root = tempfile::tempdir().unwrap();
        let (on_change, seen) = collecting();
        let watcher = ShareWatcher::start(on_change).unwrap();
        let name = root.path().to_string_lossy().into_owned();
        watcher.watch(&name);
        std::thread::sleep(Duration::from_millis(200));

        for text in ["one", "two", "three"] {
            std::fs::write(root.path().join("a.md"), text).unwrap();
        }

        assert!(wait_for(|| count(&seen) >= 1), "changement non signalé");
        std::thread::sleep(SETTLE * 2);
        assert_eq!(count(&seen), 1);
        assert_eq!(seen.lock().unwrap()[0], name);
    }

    #[test]
    fn other_files_and_temporary_files_are_not_reported() {
        let root = tempfile::tempdir().unwrap();
        let (on_change, seen) = collecting();
        let watcher = ShareWatcher::start(on_change).unwrap();
        watcher.watch(&root.path().to_string_lossy());
        std::thread::sleep(Duration::from_millis(200));

        std::fs::write(root.path().join("image.png"), "x").unwrap();
        std::fs::write(root.path().join(".daftr-sync-1-0.tmp"), "x").unwrap();
        std::thread::sleep(SETTLE * 3);

        assert_eq!(count(&seen), 0);
    }

    #[test]
    fn an_unwatched_folder_is_no_longer_reported() {
        let root = tempfile::tempdir().unwrap();
        let (on_change, seen) = collecting();
        let watcher = ShareWatcher::start(on_change).unwrap();
        let name = root.path().to_string_lossy().into_owned();
        watcher.watch(&name);
        std::thread::sleep(Duration::from_millis(200));
        watcher.unwatch(&name);
        std::thread::sleep(Duration::from_millis(100));

        std::fs::write(root.path().join("a.md"), "x").unwrap();
        std::thread::sleep(SETTLE * 3);

        assert_eq!(count(&seen), 0);
    }

    #[test]
    fn a_missing_folder_is_ignored_without_failing() {
        let (on_change, seen) = collecting();
        let watcher = ShareWatcher::start(on_change).unwrap();

        watcher.watch("/does/not/exist");
        std::thread::sleep(Duration::from_millis(200));

        assert_eq!(count(&seen), 0);
    }

    #[test]
    fn tells_relevant_paths() {
        assert!(is_relevant(&PathBuf::from("/c/docs/a.md")));
        assert!(is_relevant(&PathBuf::from("/c/docs/A.MD")));
        assert!(is_relevant(&PathBuf::from("/c/docs/folder")));
        assert!(!is_relevant(&PathBuf::from("/c/docs/a.png")));
        assert!(!is_relevant(&PathBuf::from("/c/docs/.daftr-sync-1-2.tmp")));
    }
}
