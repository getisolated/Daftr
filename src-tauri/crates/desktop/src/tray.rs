//! Icône de la zone de notification : l'app y reste quand sa fenêtre est fermée.

use domain::i18n::{Language, MessageKey, message};
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Wry};

use crate::lifecycle;
use crate::windows::{main_window, overlay};

const TRAY_ID: &str = "main";
const OPEN_APP: &str = "open-app";
const QUICK_NOTE: &str = "quick-note";
const SETTINGS: &str = "settings";
const QUIT: &str = "quit";

/// Crée l'icône et son menu.
pub fn create(app: &AppHandle, language: Language) -> tauri::Result<()> {
    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Daftr")
        .menu(&build_menu(app, language)?)
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(on_tray_icon_event);
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// Reconstruit le menu dans une autre langue.
pub fn set_language(app: &AppHandle, language: Language) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };
    let updated = build_menu(app, language).and_then(|menu| tray.set_menu(Some(menu)));
    if let Err(error) = updated {
        tracing::warn!(%error, "menu du tray non traduit");
    }
}

fn build_menu(app: &AppHandle, language: Language) -> tauri::Result<Menu<Wry>> {
    let item = |id: &str, key: MessageKey| {
        MenuItem::with_id(app, id, message(language, key), true, None::<&str>)
    };

    Menu::with_items(
        app,
        &[
            &item(OPEN_APP, MessageKey::TrayOpenApp)?,
            &item(QUICK_NOTE, MessageKey::TrayQuickNote)?,
            &item(SETTINGS, MessageKey::TraySettings)?,
            &PredefinedMenuItem::separator(app)?,
            &item(QUIT, MessageKey::TrayQuit)?,
        ],
    )
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    match event.id().as_ref() {
        OPEN_APP => main_window::show(app),
        QUICK_NOTE => overlay::show(app),
        SETTINGS => main_window::open_settings(app),
        QUIT => lifecycle::quit(app),
        other => tracing::debug!(id = other, "élément de menu inconnu"),
    }
}

fn on_tray_icon_event(tray: &TrayIcon, event: TrayIconEvent) {
    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        main_window::show(tray.app_handle());
    }
}
