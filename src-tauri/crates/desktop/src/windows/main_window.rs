//! Fenêtre principale.

use domain::settings::{AppSettings, ThemeId};
use presentation::events::OPEN_SETTINGS;
use tauri::webview::PageLoadEvent;
use tauri::window::Color;
use tauri::{
    AppHandle, Manager, Theme, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent, Wry,
};

use crate::events::emit_to;
use crate::open_args;
use crate::state::desktop;

/// Étiquette de la fenêtre principale.
pub const MAIN: &str = "main";

/// Feux tricolores de macOS (points logiques). La barre d'onglets (`tabs-bar.component.scss`)
/// fait 38 px de haut, les boutons 14 px : centrés, leur haut doit être à (38 − 14) / 2 = 12 px.
/// Le `y` de Tauri n'est pas ce haut : il fixe la hauteur du conteneur de la barre de titre
/// (bouton + `y`), et AppKit garde les boutons à ~9 px du bas de ce conteneur ; mesuré sur
/// macOS 26, `y = 21` met le haut des feux à 12 px (centre à 19 px, comme la barre). Les trois
/// feux vont alors de x = 12 à ~72 px ; la barre d'onglets leur réserve 78 px à gauche.
#[cfg(target_os = "macos")]
const TRAFFIC_LIGHTS: tauri::LogicalPosition<f64> = tauri::LogicalPosition { x: 12.0, y: 21.0 };

/// Crée la fenêtre, cachée jusqu'au chargement du renderer (`show` : la montrer alors ; un
/// lancement au démarrage de Windows reste dans le tray).
pub fn create(app: &AppHandle, show: bool) -> tauri::Result<WebviewWindow> {
    let builder = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("index.html".into()))
        .title("Daftr")
        .inner_size(1280.0, 800.0)
        .min_inner_size(600.0, 400.0)
        .visible(false)
        // Le glisser-déposer HTML5 (onglets, arborescence) exige le gestionnaire de WebView2
        .disable_drag_drop_handler()
        .on_page_load(move |window, payload| on_page_load(&window, payload.event(), show));
    let window = title_bar(builder).build()?;

    paint_theme_background(&window);
    window.on_window_event({
        let window = window.clone();
        move |event| on_window_event(&window, event)
    });
    #[cfg(debug_assertions)]
    window.open_devtools();
    Ok(window)
}

/// Sans décorations : la barre d'onglets dessine les boutons de la fenêtre.
#[cfg(not(target_os = "macos"))]
fn title_bar(
    builder: WebviewWindowBuilder<'_, Wry, AppHandle>,
) -> WebviewWindowBuilder<'_, Wry, AppHandle> {
    builder.decorations(false)
}

/// Feux tricolores natifs par-dessus la barre d'onglets, sans titre ; la barre reste la zone de
/// glisser (`data-tauri-drag-region`, double-clic = zoom).
#[cfg(target_os = "macos")]
fn title_bar(
    builder: WebviewWindowBuilder<'_, Wry, AppHandle>,
) -> WebviewWindowBuilder<'_, Wry, AppHandle> {
    builder
        .decorations(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(TRAFFIC_LIGHTS)
}

/// Montre la fenêtre principale (la recrée si elle a été détruite).
pub fn show(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN) else {
        if let Err(error) = create(app, true) {
            tracing::error!(%error, "fenêtre principale non recréée");
        }
        return;
    };
    let shown = window
        .unminimize()
        .and_then(|()| window.show())
        .and_then(|()| window.set_focus());
    if let Err(error) = shown {
        tracing::warn!(%error, "fenêtre principale non affichée");
    }
}

/// Montre la fenêtre et ouvre ses réglages.
pub fn open_settings(app: &AppHandle) {
    show(app);
    emit_to(app, MAIN, OPEN_SETTINGS, ());
}

fn on_page_load(window: &WebviewWindow, event: PageLoadEvent, show: bool) {
    if event != PageLoadEvent::Finished {
        return;
    }
    let app = window.app_handle();
    // Un rechargement du renderer ne doit ni remontrer la fenêtre ni rouvrir les fichiers
    if desktop(app).take_launch_files() {
        if show && let Err(error) = window.show() {
            tracing::warn!(%error, "fenêtre principale non affichée");
        }
        open_args::open_from_arguments(app, std::env::args());
        // Fichiers double-cliqués dans le Finder pendant le lancement
        #[cfg(target_os = "macos")]
        open_args::open_pending_files(app);
    }
}

/// Fermer la fenêtre la cache seulement : l'app vit dans le tray.
fn on_window_event(window: &WebviewWindow, event: &WindowEvent) {
    if let WindowEvent::CloseRequested { api, .. } = event
        && !desktop(window.app_handle()).is_quitting()
    {
        api.prevent_close();
        if let Err(error) = window.hide() {
            tracing::warn!(%error, "fenêtre principale non cachée");
        }
    }
}

/// Fond de la fenêtre avant que le renderer ne peigne : celui du thème, pour qu'un thème
/// clair ne clignote pas en sombre.
fn paint_theme_background(window: &WebviewWindow) {
    let os_is_dark = window.theme().is_ok_and(|theme| theme == Theme::Dark);
    let settings = desktop(window.app_handle()).services.settings.current();
    let (red, green, blue) = theme(&settings, os_is_dark).background_rgb();

    if let Err(error) = window.set_background_color(Some(Color(red, green, blue, 255))) {
        tracing::warn!(%error, "fond de la fenêtre non appliqué");
    }
}

fn theme(settings: &AppSettings, os_is_dark: bool) -> ThemeId {
    settings.theme.resolve(
        os_is_dark,
        settings.preferred_dark_theme,
        settings.preferred_light_theme,
    )
}
