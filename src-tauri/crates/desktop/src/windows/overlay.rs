//! Overlay de note rapide : petite fenêtre translucide, toujours au premier plan.
//!
//! Créée cachée au démarrage pour s'afficher instantanément ; recréée si le matériau
//! utilisable (acrylique sous Windows, vibrancy claire ou sombre sous macOS, ou panneau
//! translucide) a changé depuis.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use app::settings::SettingsChange;
use domain::overlay::{Size, anchor_bottom_right, resolve_overlay_position};
use domain::settings::ThemeKind;
use platform::sync::lock;
use presentation::events::OVERLAY_SHOWN;
use tauri::window::Color;
use tauri::{
    AppHandle, LogicalPosition, LogicalSize, Manager, Theme, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};
use window_vibrancy::{NSVisualEffectMaterial, NSVisualEffectState};

use super::main_window::MAIN;
use super::{acrylic, geometry};
use crate::events::emit_to;
use crate::state::desktop;

/// Étiquette de l'overlay (le renderer choisit sa racine d'après elle).
pub const OVERLAY: &str = "overlay";

/// `dragging` déclenche `Moved` en continu : la position n'est enregistrée qu'au repos.
const POSITION_SAVE_DELAY: Duration = Duration::from_millis(300);

/// Délai entre deux vérifications après une perte de focus.
const BLUR_CHECK_INTERVAL: Duration = Duration::from_millis(250);

/// Un déplacement plus récent : la fenêtre est en train d'être glissée par sa poignée.
const DRAG_GRACE: Duration = Duration::from_millis(400);

/// Rayon des coins de la vibrancy macOS, en points (celui des coins de `overlay.component.scss`).
const VIBRANCY_CORNER_RADIUS: f64 = 12.0;

/// Le renderer ne peint alors aucun fond et laisse voir l'acrylique ou la vibrancy.
const ACRYLIC_SCRIPT: &str = "document.addEventListener('DOMContentLoaded', () => \
                              document.documentElement.classList.add('overlay-acrylic'));";

/// État de l'overlay.
#[derive(Default)]
pub struct OverlayState {
    /// Matériau de la fenêtre actuelle (il ne change pas après sa création).
    material: Mutex<Option<Material>>,
    /// Numéro du dernier déplacement : seul le plus récent enregistre la position.
    move_generation: AtomicU64,
    /// Moment du dernier déplacement (glisser la poignée retire un instant le focus).
    last_moved: Mutex<Option<Instant>>,
    /// Une perte de focus est en cours d'examen.
    watching_blur: AtomicBool,
}

/// Fond natif de la fenêtre de l'overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Material {
    /// Acrylique de Windows (flou de ce qui est derrière la fenêtre).
    Acrylic,
    /// Panneau translucide peint par le renderer, sans effet natif.
    Translucent,
    /// Vibrancy macOS sombre (`HudWindow`), pour un thème sombre.
    VibrancyDark,
    /// Vibrancy macOS claire (`Popover`), pour un thème clair.
    VibrancyLight,
}

impl Material {
    /// Un effet natif est dessous : le renderer ne peint pas de fond et la fenêtre a une ombre.
    const fn has_native_effect(self) -> bool {
        !matches!(self, Self::Translucent)
    }
}

/// Système qui affiche l'overlay (seuls Windows et macOS ont un effet natif).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Platform {
    Windows,
    MacOs,
    Other,
}

impl Platform {
    const CURRENT: Self = if cfg!(windows) {
        Self::Windows
    } else if cfg!(target_os = "macos") {
        Self::MacOs
    } else {
        Self::Other
    };
}

/// Matériau de l'overlay : sur macOS, la vibrancy suit le thème résolu de l'application (le
/// thème n'a pas d'effet ailleurs) ; sinon l'acrylique s'il est rendu.
const fn material_for(platform: Platform, effect_active: bool, theme: ThemeKind) -> Material {
    match (platform, effect_active, theme) {
        (_, false, _) => Material::Translucent,
        (Platform::MacOs, true, ThemeKind::Dark) => Material::VibrancyDark,
        (Platform::MacOs, true, ThemeKind::Light) => Material::VibrancyLight,
        (Platform::Windows | Platform::Other, true, _) => Material::Acrylic,
    }
}

/// Matériau utilisable maintenant (la sonde de l'acrylique peut durer sous Windows).
fn current_material(app: &AppHandle) -> Material {
    let platform = Platform::CURRENT;
    let theme = match platform {
        Platform::MacOs => app_theme_kind(app),
        // Sans effet hors macOS : l'OS n'est pas interrogé
        Platform::Windows | Platform::Other => ThemeKind::Dark,
    };
    material_for(platform, acrylic::is_active(), theme)
}

/// Famille du thème affiché par l'application, résolu comme la fenêtre principale le fait
/// (`main_window::paint_theme_background`), avec le mode de l'OS lu sur la fenêtre principale
/// (l'overlay à défaut). L'apparence de l'overlay n'est pas forcée par `theme()` : sous macOS,
/// tao l'applique à toute l'application, fenêtre principale comprise.
fn app_theme_kind(app: &AppHandle) -> ThemeKind {
    let os_is_dark = app
        .get_webview_window(MAIN)
        .or_else(|| app.get_webview_window(OVERLAY))
        .and_then(|window| window.theme().ok())
        .is_some_and(|theme| theme == Theme::Dark);
    let settings = desktop(app).services.settings.current();

    settings
        .theme
        .resolve(
            os_is_dark,
            settings.preferred_dark_theme,
            settings.preferred_light_theme,
        )
        .kind()
}

/// Suite à donner à une perte de focus de l'overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlurOutcome {
    /// Le focus est revenu : rien à faire.
    Keep,
    /// La fenêtre est glissée : on attend la fin du déplacement.
    Wait,
    /// Un clic ailleurs sur l'écran : l'overlay se cache.
    Hide,
}

/// Glisser la fenêtre par sa poignée passe par la boucle de déplacement de Windows, qui retire
/// le focus le temps du geste : l'overlay ne se cache que s'il ne l'a pas retrouvé et ne bouge
/// plus.
fn blur_outcome(focused: bool, moved_recently: bool) -> BlurOutcome {
    match (focused, moved_recently) {
        (true, _) => BlurOutcome::Keep,
        (false, true) => BlurOutcome::Wait,
        (false, false) => BlurOutcome::Hide,
    }
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

/// L'overlay existant, ou recréé si le matériau utilisable a changé (économiseur de batterie
/// sous Windows, passage clair/sombre sous macOS).
fn ensure_window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let wanted = current_material(app);
    let state = desktop(app);
    let mut material = lock(&state.overlay.material);

    if let Some(window) = app.get_webview_window(OVERLAY) {
        if *material == Some(wanted) {
            return Ok(window);
        }
        window.destroy()?;
    }
    let window = create(app, wanted)?;
    *material = Some(wanted);
    Ok(window)
}

fn create(app: &AppHandle, material: Material) -> tauri::Result<WebviewWindow> {
    let size = desktop(app).services.settings.current().overlay_size();
    let mut builder = WebviewWindowBuilder::new(app, OVERLAY, WebviewUrl::App("index.html".into()))
        .title("Quick note")
        .inner_size(f64::from(size.width), f64::from(size.height))
        .visible(false)
        .decorations(false)
        .transparent(true)
        .background_color(Color(0, 0, 0, 0))
        .shadow(material.has_native_effect())
        .resizable(false)
        .minimizable(false)
        .maximizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .disable_drag_drop_handler();
    if material.has_native_effect() {
        builder = builder.initialization_script(ACRYLIC_SCRIPT);
    }
    // La note rapide suit l'utilisateur d'un Space à l'autre (`skip_taskbar` est ignoré ici)
    #[cfg(target_os = "macos")]
    {
        builder = builder.visible_on_all_workspaces(true);
    }

    let window = builder.build()?;
    apply_material(&window, material);
    window.on_window_event({
        let window = window.clone();
        move |event| on_window_event(&window, event)
    });
    Ok(window)
}

fn apply_material(window: &WebviewWindow, material: Material) {
    match material {
        Material::Translucent => {}
        Material::Acrylic => {
            if let Err(error) = window_vibrancy::apply_acrylic(window, None) {
                tracing::warn!(%error, "acrylique non appliqué, panneau translucide");
            }
        }
        Material::VibrancyDark => apply_vibrancy(window, NSVisualEffectMaterial::HudWindow),
        Material::VibrancyLight => apply_vibrancy(window, NSVisualEffectMaterial::Popover),
    }
}

/// AppKit n'accepte la vue de flou que sur le fil principal, alors que l'overlay est créé en
/// arrière-plan : la pose passe par la boucle d'événements, avant tout `show` demandé ensuite.
fn apply_vibrancy(window: &WebviewWindow, effect: NSVisualEffectMaterial) {
    let target = window.clone();
    let scheduled = window.run_on_main_thread(move || {
        let applied = window_vibrancy::apply_vibrancy(
            &target,
            effect,
            Some(NSVisualEffectState::Active),
            Some(VIBRANCY_CORNER_RADIUS),
        );
        if let Err(error) = applied {
            tracing::warn!(%error, "vibrancy non appliquée, fond transparent");
        }
    });
    if let Err(error) = scheduled {
        tracing::warn!(%error, "vibrancy non programmée");
    }
}

fn on_window_event(window: &WebviewWindow, event: &WindowEvent) {
    match event {
        // Un clic ailleurs sur l'écran cache l'overlay (le brouillon reste), pas un glisser
        WindowEvent::Focused(false) => watch_blur(window),
        WindowEvent::Moved(physical) => {
            *lock(&desktop(window.app_handle()).overlay.last_moved) = Some(Instant::now());
            let position = geometry::logical_position(window, *physical);
            schedule_position_save(window, position);
        }
        _ => {}
    }
}

/// Examine une perte de focus en arrière-plan, une seule à la fois.
fn watch_blur(window: &WebviewWindow) {
    if desktop(window.app_handle())
        .overlay
        .watching_blur
        .swap(true, Ordering::Relaxed)
    {
        return;
    }
    let window = window.clone();

    spawn("overlay-blur", move || {
        settle_blur(&window);
        desktop(window.app_handle())
            .overlay
            .watching_blur
            .store(false, Ordering::Relaxed);
    });
}

fn settle_blur(window: &WebviewWindow) {
    loop {
        std::thread::sleep(BLUR_CHECK_INTERVAL);
        let focused = window.is_focused().unwrap_or(false);

        match blur_outcome(focused, moved_recently(window)) {
            BlurOutcome::Keep => return,
            BlurOutcome::Wait => {}
            BlurOutcome::Hide => return hide(window.app_handle()),
        }
    }
}

fn moved_recently(window: &WebviewWindow) -> bool {
    let last_moved = *lock(&desktop(window.app_handle()).overlay.last_moved);
    last_moved.is_some_and(|moment| moment.elapsed() < DRAG_GRACE)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn windows_uses_acrylic_only_when_it_is_rendered_whatever_the_theme() {
        for theme in [ThemeKind::Dark, ThemeKind::Light] {
            assert_eq!(
                material_for(Platform::Windows, true, theme),
                Material::Acrylic
            );
            assert_eq!(
                material_for(Platform::Windows, false, theme),
                Material::Translucent
            );
        }
    }

    #[test]
    fn macos_vibrancy_follows_the_resolved_theme() {
        assert_eq!(
            material_for(Platform::MacOs, true, ThemeKind::Dark),
            Material::VibrancyDark
        );
        assert_eq!(
            material_for(Platform::MacOs, true, ThemeKind::Light),
            Material::VibrancyLight
        );
        assert_eq!(
            material_for(Platform::MacOs, false, ThemeKind::Dark),
            Material::Translucent
        );
    }

    #[test]
    fn other_systems_keep_the_translucent_panel() {
        assert_eq!(
            material_for(Platform::Other, false, ThemeKind::Dark),
            Material::Translucent
        );
    }

    #[test]
    fn only_native_effects_hide_the_renderer_background() {
        assert!(Material::Acrylic.has_native_effect());
        assert!(Material::VibrancyDark.has_native_effect());
        assert!(Material::VibrancyLight.has_native_effect());
        assert!(!Material::Translucent.has_native_effect());
    }

    #[test]
    fn a_theme_switch_on_macos_changes_the_material() {
        assert_ne!(
            material_for(Platform::MacOs, true, ThemeKind::Dark),
            material_for(Platform::MacOs, true, ThemeKind::Light)
        );
    }

    #[test]
    fn hides_only_after_a_real_blur() {
        assert_eq!(blur_outcome(true, false), BlurOutcome::Keep);
        assert_eq!(blur_outcome(true, true), BlurOutcome::Keep);
        assert_eq!(blur_outcome(false, true), BlurOutcome::Wait);
        assert_eq!(blur_outcome(false, false), BlurOutcome::Hide);
    }
}
