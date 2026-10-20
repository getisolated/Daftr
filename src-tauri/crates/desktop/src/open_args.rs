//! Fichiers markdown passés en argument (association de fichiers, second lancement) ou, sur
//! macOS, envoyés par le Finder (`RunEvent::Opened`).

#[cfg(target_os = "macos")]
use std::path::Path;
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::sync::Mutex;

use domain::collection::is_markdown_file;
#[cfg(target_os = "macos")]
use platform::sync::lock;
use presentation::dto::CollectionFileRefDto;
use presentation::events::COLLECTIONS_OPEN;
use tauri::AppHandle;
#[cfg(target_os = "macos")]
use tauri::{Manager, Url};

use crate::events::emit_to;
use crate::state::desktop;
use crate::windows::main_window::{self, MAIN};

/// Un second lancement (menu Démarrer, `.md` double-cliqué…) ramène l'instance en cours.
pub fn on_second_instance(app: &AppHandle, args: Vec<String>, _cwd: String) {
    main_window::show(app);
    open_from_arguments(app, args.into_iter());
}

/// Ouvre les fichiers markdown des arguments comme collections, dans des onglets.
pub fn open_from_arguments(app: &AppHandle, args: impl Iterator<Item = String>) {
    open_files(app, &markdown_files(args));
}

/// Fichiers envoyés par le Finder (macOS). Au lancement par double-clic, AppKit les livre
/// avant `applicationDidFinishLaunching`, donc avant `setup` (et `DesktopState`) : ils
/// attendent alors `open_pending_files`, dans une file gérée à part.
#[cfg(target_os = "macos")]
pub fn open_from_urls(app: &AppHandle, urls: &[Url]) {
    let files = markdown_files_from_urls(urls);
    if files.is_empty() {
        return;
    }
    let count = files.len();
    let ready = lock(&opened_files(app).0).receive(files);
    tracing::info!(
        count,
        waiting = ready.is_empty(),
        "fichiers reçus du Finder"
    );
    if !ready.is_empty() {
        main_window::show(app);
        open_files(app, &ready);
    }
}

/// Ouvre les fichiers reçus du Finder pendant le chargement de la fenêtre.
#[cfg(target_os = "macos")]
pub fn open_pending_files(app: &AppHandle) {
    let files = lock(&opened_files(app).0).mark_ready();
    if files.is_empty() {
        return;
    }
    tracing::info!(count = files.len(), "fichiers en attente ouverts");
    // Même lancée cachée (ouverture de session), l'app montre les fichiers demandés
    main_window::show(app);
    open_files(app, &files);
}

/// File des fichiers reçus du Finder, gérée par Tauri hors de `DesktopState`.
#[cfg(target_os = "macos")]
#[derive(Default)]
struct OpenedFilesState(Mutex<OpenedFiles>);

/// La file, créée au premier besoin (`manage` ne remplace pas un état déjà géré).
#[cfg(target_os = "macos")]
fn opened_files(app: &AppHandle) -> tauri::State<'_, OpenedFilesState> {
    app.manage(OpenedFilesState::default());
    app.state::<OpenedFilesState>()
}

fn open_files(app: &AppHandle, files: &[PathBuf]) {
    if files.is_empty() {
        return;
    }
    match presentation::collections::open_paths(&desktop(app).services.collections, files) {
        Ok(refs) => send_open_files(app, refs),
        Err(error) => tracing::warn!(message = %error.message, "fichiers non ouverts"),
    }
}

/// Demande à la fenêtre d'ouvrir ces fichiers.
pub fn send_open_files(app: &AppHandle, refs: Vec<CollectionFileRefDto>) {
    if !refs.is_empty() {
        emit_to(app, MAIN, COLLECTIONS_OPEN, refs);
    }
}

/// Les arguments (sans l'exécutable) qui sont des fichiers markdown existants.
fn markdown_files(args: impl Iterator<Item = String>) -> Vec<PathBuf> {
    args.skip(1)
        .filter(|arg| is_markdown_file(arg))
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .collect()
}

/// Les URL `file://` qui désignent des fichiers markdown existants.
#[cfg(target_os = "macos")]
fn markdown_files_from_urls(urls: &[Url]) -> Vec<PathBuf> {
    urls.iter()
        .filter_map(url_to_path)
        .filter(|path| is_markdown_path(path) && path.is_file())
        .collect()
}

/// Le chemin local d'une URL `file://` (les autres schémas sont ignorés).
#[cfg(target_os = "macos")]
fn url_to_path(url: &Url) -> Option<PathBuf> {
    if url.scheme() != "file" {
        return None;
    }
    url.to_file_path().ok()
}

#[cfg(target_os = "macos")]
fn is_markdown_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(is_markdown_file)
}

/// Fichiers reçus du Finder avant que la fenêtre ait fini de charger (macOS).
#[cfg(target_os = "macos")]
#[derive(Debug, Default)]
struct OpenedFiles {
    /// La fenêtre a chargé : les fichiers s'ouvrent directement.
    ready: bool,
    /// Fichiers en attente du chargement.
    pending: Vec<PathBuf>,
}

#[cfg(target_os = "macos")]
impl OpenedFiles {
    /// Les fichiers à ouvrir tout de suite ; tant que la fenêtre charge, aucun (ils attendent).
    fn receive(&mut self, files: Vec<PathBuf>) -> Vec<PathBuf> {
        if self.ready {
            return files;
        }
        self.pending.extend(files);
        Vec::new()
    }

    /// La fenêtre a chargé : vide la file d'attente.
    fn mark_ready(&mut self) -> Vec<PathBuf> {
        self.ready = true;
        std::mem::take(&mut self.pending)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_existing_markdown_files_only() {
        let directory = tempfile_dir("args");
        let note = directory.join("note.md");
        std::fs::write(&note, "").expect("écriture");

        let args = [
            "app.exe",
            "--hidden",
            "missing.md",
            note.to_str().unwrap_or_default(),
            "a.txt",
        ];
        let files = markdown_files(args.into_iter().map(str::to_owned));

        assert_eq!(files, [note]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn converts_file_urls_to_paths() {
        let url = Url::parse("file:///Users/me/Notes/R%C3%A9union%20du%20jour.md").expect("URL");
        assert_eq!(
            url_to_path(&url),
            Some(PathBuf::from("/Users/me/Notes/Réunion du jour.md"))
        );

        let web = Url::parse("https://example.com/note.md").expect("URL");
        assert_eq!(url_to_path(&web), None);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn keeps_existing_markdown_urls_only() {
        let directory = tempfile_dir("urls");
        let note = directory.join("note ç.MD");
        let text = directory.join("a.txt");
        std::fs::write(&note, "").expect("écriture");
        std::fs::write(&text, "").expect("écriture");

        let urls = [
            Url::from_file_path(&note).expect("URL"),
            Url::from_file_path(&text).expect("URL"),
            Url::from_file_path(directory.join("missing.md")).expect("URL"),
            Url::from_directory_path(&directory).expect("URL"),
            Url::parse("daftr://note.md").expect("URL"),
        ];

        assert_eq!(markdown_files_from_urls(&urls), [note]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn queues_files_until_the_window_has_loaded() {
        let mut opened = OpenedFiles::default();

        assert!(opened.receive(vec![PathBuf::from("/a.md")]).is_empty());
        assert!(opened.receive(vec![PathBuf::from("/b.md")]).is_empty());
        assert_eq!(
            opened.mark_ready(),
            [PathBuf::from("/a.md"), PathBuf::from("/b.md")]
        );
        assert!(opened.mark_ready().is_empty());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn opens_files_directly_once_the_window_has_loaded() {
        let mut opened = OpenedFiles::default();
        assert!(opened.mark_ready().is_empty());

        assert_eq!(
            opened.receive(vec![PathBuf::from("/c.md")]),
            [PathBuf::from("/c.md")]
        );
        assert!(opened.mark_ready().is_empty());
    }

    fn tempfile_dir(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("daftr-open-args-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("dossier temporaire");
        directory
    }
}
