//! État partagé de l'application, géré par Tauri (`app.state::<DesktopState>()`).

use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "macos")]
use std::sync::{Mutex, PoisonError};

use presentation::Services;
use tauri::{AppHandle, Manager};

#[cfg(target_os = "macos")]
use crate::open_args::OpenedFiles;
use crate::shortcut::ShortcutState;
use crate::updater::UpdateState;
use crate::windows::overlay::OverlayState;

/// Services et état des fenêtres.
pub struct DesktopState {
    /// Cas d'usage.
    pub services: Services,
    /// Fermer la fenêtre la cache dans le tray ; seul un vrai « Quitter » ferme l'app.
    quitting: AtomicBool,
    /// Les fichiers passés au lancement ne s'ouvrent qu'au premier chargement de la fenêtre.
    launch_files_opened: AtomicBool,
    /// Fichiers reçus du Finder (`RunEvent::Opened`), en attente du chargement de la fenêtre.
    #[cfg(target_os = "macos")]
    opened_files: Mutex<OpenedFiles>,
    /// Overlay de note rapide.
    pub overlay: OverlayState,
    /// Raccourci global enregistré.
    pub shortcut: ShortcutState,
    /// Mise à jour trouvée / téléchargée.
    pub updates: UpdateState,
}

impl DesktopState {
    /// État initial.
    pub fn new(services: Services) -> Self {
        Self {
            services,
            quitting: AtomicBool::new(false),
            launch_files_opened: AtomicBool::new(false),
            #[cfg(target_os = "macos")]
            opened_files: Mutex::default(),
            overlay: OverlayState::default(),
            shortcut: ShortcutState::default(),
            updates: UpdateState::default(),
        }
    }

    /// L'application est en train de quitter.
    pub fn is_quitting(&self) -> bool {
        self.quitting.load(Ordering::Relaxed)
    }

    /// Marque l'application comme quittant : les fenêtres se ferment vraiment.
    pub fn set_quitting(&self) {
        self.quitting.store(true, Ordering::Relaxed);
    }

    /// `true` la première fois seulement.
    pub fn take_launch_files(&self) -> bool {
        !self.launch_files_opened.swap(true, Ordering::Relaxed)
    }

    /// Fichiers reçus du Finder : ceux à ouvrir tout de suite (vide tant que la fenêtre
    /// n'a pas fini de charger, ils sont alors mis en attente).
    #[cfg(target_os = "macos")]
    pub fn receive_opened_files(&self, files: Vec<std::path::PathBuf>) -> Vec<std::path::PathBuf> {
        self.opened_files
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .receive(files)
    }

    /// La fenêtre a fini de charger : les fichiers en attente, à ouvrir maintenant.
    #[cfg(target_os = "macos")]
    pub fn take_pending_opened_files(&self) -> Vec<std::path::PathBuf> {
        self.opened_files
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .mark_ready()
    }
}

/// L'état de l'application (géré dès `setup`).
pub fn desktop(app: &AppHandle) -> tauri::State<'_, DesktopState> {
    app.state::<DesktopState>()
}
