//! Menu d'application de macOS (barre de menus en haut de l'écran).
//!
//! Le menu Édition est indispensable : dans WKWebView, ⌘C, ⌘V, ⌘X, ⌘A et ⌘Z passent par les
//! éléments prédéfinis du menu. « Quitter » est un élément à nous plutôt que l'élément prédéfini,
//! pour quitter par le même chemin que le tray (`lifecycle::quit`).
//!
//! Aucun accélérateur ne recouvre un raccourci du renderer (⌘N, ⌘S, ⌘F, ⌘, ⌘W…) : un
//! accélérateur de menu natif le capterait avant la page. D'où « Paramètres… » sans
//! accélérateur et « Fermer la fenêtre » à nous en ⇧⌘W (le prédéfini porte ⌘W, qui ferme
//! l'onglet dans l'app).

use domain::i18n::{Language, MessageKey, message};
use tauri::menu::{
    AboutMetadata, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu, WINDOW_SUBMENU_ID,
};
use tauri::{AppHandle, Manager, Wry};

use crate::lifecycle;
use crate::windows::main_window::{self, MAIN};
use crate::windows::overlay::{self, OVERLAY};

/// Identifiants propres au menu d'application (distincts de ceux du tray : les deux
/// gestionnaires reçoivent tous les événements de menu).
const QUIT: &str = "app-menu-quit";
const SETTINGS: &str = "app-menu-settings";
const CLOSE_WINDOW: &str = "app-menu-close-window";

/// Installe le menu dans `language` et écoute ses éléments.
pub fn install(app: &AppHandle, language: Language) -> tauri::Result<()> {
    app.set_menu(build(app, language)?)?;
    app.on_menu_event(on_menu_event);
    Ok(())
}

/// Reconstruit le menu dans une autre langue.
pub fn set_language(app: &AppHandle, language: Language) {
    if let Err(error) = build(app, language).and_then(|menu| app.set_menu(menu)) {
        tracing::warn!(%error, "menu d'application non traduit");
    }
}

fn build(app: &AppHandle, language: Language) -> tauri::Result<Menu<Wry>> {
    let text = |key: MessageKey| Some(message(language, key));
    let separator = || PredefinedMenuItem::separator(app);
    let package = app.package_info();
    let about = AboutMetadata {
        name: Some(package.name.clone()),
        version: Some(package.version.to_string()),
        ..AboutMetadata::default()
    };

    // macOS remplace le titre du premier sous-menu par le nom de l'application
    let app_menu = Submenu::with_items(
        app,
        &package.name,
        true,
        &[
            &PredefinedMenuItem::about(app, text(MessageKey::MenuAbout), Some(about))?,
            &separator()?,
            &MenuItem::with_id(
                app,
                SETTINGS,
                message(language, MessageKey::TraySettings),
                true,
                None::<&str>,
            )?,
            &separator()?,
            &PredefinedMenuItem::services(app, text(MessageKey::MenuServices))?,
            &separator()?,
            &PredefinedMenuItem::hide(app, text(MessageKey::MenuHide))?,
            &PredefinedMenuItem::hide_others(app, text(MessageKey::MenuHideOthers))?,
            &PredefinedMenuItem::show_all(app, text(MessageKey::MenuShowAll))?,
            &separator()?,
            &MenuItem::with_id(
                app,
                QUIT,
                message(language, MessageKey::MenuQuit),
                true,
                Some("CmdOrCtrl+Q"),
            )?,
        ],
    )?;

    let edit_menu = Submenu::with_items(
        app,
        message(language, MessageKey::MenuEdit),
        true,
        &[
            &PredefinedMenuItem::undo(app, text(MessageKey::MenuUndo))?,
            &PredefinedMenuItem::redo(app, text(MessageKey::MenuRedo))?,
            &separator()?,
            &PredefinedMenuItem::cut(app, text(MessageKey::MenuCut))?,
            &PredefinedMenuItem::copy(app, text(MessageKey::MenuCopy))?,
            &PredefinedMenuItem::paste(app, text(MessageKey::MenuPaste))?,
            &PredefinedMenuItem::select_all(app, text(MessageKey::MenuSelectAll))?,
        ],
    )?;

    // Cet identifiant en fait le menu Fenêtre de NSApp (liste des fenêtres, plein écran)
    let window_menu = Submenu::with_id_and_items(
        app,
        WINDOW_SUBMENU_ID,
        message(language, MessageKey::MenuWindow),
        true,
        &[
            &PredefinedMenuItem::minimize(app, text(MessageKey::MenuMinimize))?,
            &PredefinedMenuItem::maximize(app, text(MessageKey::MenuZoom))?,
            &separator()?,
            &MenuItem::with_id(
                app,
                CLOSE_WINDOW,
                message(language, MessageKey::MenuCloseWindow),
                true,
                Some("CmdOrCtrl+Shift+W"),
            )?,
        ],
    )?;

    Menu::with_items(app, &[&app_menu, &edit_menu, &window_menu])
}

/// Les éléments du tray arrivent aussi ici : on les ignore.
fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        QUIT => lifecycle::quit(app),
        SETTINGS => main_window::open_settings(app),
        CLOSE_WINDOW => close_focused_window(app),
        _ => {}
    }
}

/// Ferme la fenêtre active comme son bouton rouge : la fenêtre principale se cache (via
/// `CloseRequested`), l'overlay aussi.
fn close_focused_window(app: &AppHandle) {
    let focused = app
        .webview_windows()
        .into_values()
        .find(|window| window.is_focused().unwrap_or(false));
    let Some(window) = focused else {
        return;
    };
    match window.label() {
        OVERLAY => overlay::hide(app),
        MAIN => {
            if let Err(error) = window.close() {
                tracing::warn!(%error, "fenêtre principale non fermée");
            }
        }
        _ => {}
    }
}
