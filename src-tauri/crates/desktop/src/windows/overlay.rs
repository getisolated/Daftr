//! Overlay de note rapide : petite fenêtre translucide, toujours au premier plan.
//!
//! Créée cachée au démarrage pour s'afficher instantanément ; recréée si le matériau
//! utilisable (acrylique ou panneau translucide) a changé depuis.

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use app::settings::SettingsChange;
use domain::overlay::{Size, anchor_bottom_right, resolve_overlay_position};
use platform::sync::lock;
use presentation::events::OVERLAY_SHOWN;
use tauri::window::Color;
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};

use super::{acrylic, geometry};
use crate::events::emit_to;
use crate::state::desktop;

/// Étiquette de l'overlay (le renderer choisit sa racine d'après elle).
pub const OVERLAY: &str = "overlay";

/// `dragging` déclenche `Moved` en continu : la position n'est enregistrée qu'au repos.
const POSITION_SAVE_DELAY: Duration = Duration::from_millis(300);

/// Le renderer ne peint alors aucun fond et laisse voir l'acrylique.
const ACRYLIC_SCRIPT: &str = "document.addEventListener('DOMContentLoaded', () => \
                              document.documentElement.classList.add('overlay-acrylic'));";

/// État de l'overlay.
#[derive(Default)]
pub struct OverlayState {
    /// Matériau de la fenêtre actuelle (il ne change pas après sa création).
    acrylic: Mutex<Option<bool>>,
    /// Numéro du dernier déplacement : seul le plus récent enregistre la position.
    move_generation: AtomicU64,
}

/// Crée l'overlay caché sans retarder le démarrage (la sonde de l'acrylique peut durer).
pub fn prepare_in_background(app: &AppHandle) {
    let app = app.clone();
    spawn("overlay-prepare", move || {
        if let Err(error) = ensure_window(&app) {
            tracing::error!(%error, "overlay non créé");
        }
    });
}

/// Montre l'overlay à sa place, en arrière-plan (création et sonde peuvent attendre).
pub fn show(app: &AppHandle) {
    let app = app.clone();
    spawn("overlay-show", move || {
        if let Err(error) = show_now(&app) {
            tracing::error!(%error, "overlay non affiché");
        }
    });
}

/// Cache l'overlay ; le renderer garde le brouillon.
pub fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(OVERLAY)
        && let Err(error) = window.hide()
    {
        tracing::warn!(%error, "overlay non caché");
    }
}

/// Montre ou cache l'overlay.
pub fn toggle(app: &AppHandle) {
    let visible = app
        .get_webview_window(OVERLAY)
        .is_some_and(|window| window.is_visible().unwrap_or(false));
    if visible { hide(app) } else { show(app) }
}

/// Applique une nouvelle taille en gardant le coin bas-droit en place, et la mémorise.
pub fn apply_size(app: &AppHandle, change: &SettingsChange) {
    let Some(window) = app.get_webview_window(OVERLAY) else {
        return;
    };
    let Some(position) = geometry::current_position(&window) else {
        return;
    };
    let (old_size, new_size) = (change.before.overlay_size(), change.after.overlay_size());
    let anchored = anchor_bottom_right(position, old_size, new_size);

    set_bounds(&window, anchored, new_size);
    desktop(app)
        .services
        .settings
        .set_overlay_position(anchored);
}

fn show_now(app: &AppHandle) -> tauri::Result<()> {
    let window = ensure_window(app)?;

    place(&window);
    window.show()?;
    window.set_focus()?;
    emit_to(app, OVERLAY, OVERLAY_SHOWN, ());
    // Un changement d'économiseur de batterie est pris en compte au prochain affichage
    acrylic::refresh_in_background();
    Ok(())
}

/// L'overlay existant, ou recréé si le matériau utilisable a changé.
fn ensure_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let use_acrylic = acrylic::is_active();
    let state = desktop(app);
    let mut material = lock(&state.overlay.acrylic);

    if let Some(window) = app.get_webview_window(OVERLAY) {
        if *material == Some(use_acrylic) {
            return Ok(window);
        }
        window.destroy()?;
    }
    let window = create(app, use_acrylic)?;
    *material = Some(use_acrylic);
    Ok(window)
}

fn create(app: &AppHandle, use_acrylic: bool) -> tauri::Result<WebviewWindow> {
    let size = desktop(app).services.settings.current().overlay_size();
    let mut builder = WebviewWindowBuilder::new(app, OVERLAY, WebviewUrl::App("index.html".into()))
        .title("Quick note")
        .inner_size(f64::from(size.width), f64::from(size.height))
        .visible(false)
        .decorations(false)
        .transparent(true)
        .background_color(Color(0, 0, 0, 0))
        .shadow(use_acrylic)
        .resizable(false)
        .minimizable(false)
        .maximizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .disable_drag_drop_handler();
    if use_acrylic {
        builder = builder.initialization_script(ACRYLIC_SCRIPT);
    }

    let window = builder.build()?;
    apply_material(&window, use_acrylic);
    window.on_window_event({
        let window = window.clone();
        move |event| on_window_event(&window, event)
    });
    Ok(window)
}

fn apply_material(window: &WebviewWindow, use_acrylic: bool) {
    if use_acrylic && let Err(error) = window_vibrancy::apply_acrylic(window, None) {
        tracing::warn!(%error, "acrylique non appliqué, panneau translucide");
    }
}

fn on_window_event(window: &WebviewWindow, event: &WindowEvent) {
    match event {
        // Un clic ailleurs sur l'écran : l'overlay se cache, le brouillon reste
        WindowEvent::Focused(false) => hide(window.app_handle()),
        WindowEvent::Moved(physical) => {
            let position = geometry::logical_position(window, *physical);
            schedule_position_save(window, position);
        }
        _ => {}
    }
}

fn schedule_position_save(window: &WebviewWindow, position: domain::overlay::Position) {
    let app = window.app_handle().clone();
    let generation = desktop(&app)
        .overlay
        .move_generation
        .fetch_add(1, Ordering::Relaxed)
        + 1;
    let window = window.clone();

    spawn("overlay-position", move || {
        std::thread::sleep(POSITION_SAVE_DELAY);
        let latest = desktop(&app)
            .overlay
            .move_generation
            .load(Ordering::Relaxed)
            == generation;
        if latest && window.is_visible().unwrap_or(false) {
            desktop(&app)
                .services
                .settings
                .set_overlay_position(position);
        }
    });
}

/// Place l'overlay à sa position sauvegardée si elle est visible, sinon en bas à droite.
fn place(window: &WebviewWindow) {
    let settings = desktop(window.app_handle()).services.settings.current();
    let Some(screens) = geometry::screens(window) else {
        return;
    };
    let size = settings.overlay_size();
    let position = resolve_overlay_position(
        settings.overlay_position,
        size,
        &screens.work_areas,
        screens.primary,
    );

    set_bounds(window, position, size);
}

fn set_bounds(window: &WebviewWindow, position: domain::overlay::Position, size: Size) {
    let resized = window.set_size(LogicalSize::new(
        f64::from(size.width),
        f64::from(size.height),
    ));
    let moved = window.set_position(LogicalPosition::new(
        f64::from(position.x),
        f64::from(position.y),
    ));

    if let Err(error) = resized.and(moved) {
        tracing::warn!(%error, "overlay non placé");
    }
}

fn spawn(name: &str, job: impl FnOnce() + Send + 'static) {
    if let Err(error) = std::thread::Builder::new().name(name.to_owned()).spawn(job) {
        tracing::error!(%error, name, "thread non démarré");
    }
}
