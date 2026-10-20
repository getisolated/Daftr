//! Fichiers markdown passés en argument (association de fichiers, second lancement).

use std::path::PathBuf;

use domain::collection::is_markdown_file;
use presentation::dto::CollectionFileRefDto;
use presentation::events::COLLECTIONS_OPEN;
use tauri::AppHandle;

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
    let files = markdown_files(args);
    if files.is_empty() {
        return;
    }
    match presentation::collections::open_paths(&desktop(app).services.collections, &files) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_existing_markdown_files_only() {
        let directory = tempfile_dir();
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

    fn tempfile_dir() -> PathBuf {
        let directory = std::env::temp_dir().join(format!("daftr-open-args-{}", std::process::id()));
        std::fs::create_dir_all(&directory).expect("dossier temporaire");
        directory
    }
}
