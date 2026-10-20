//! Écrans vus en coordonnées logiques, celles du domaine et de `settings.json`.

use domain::overlay::{Position, Rect};
use tauri::{Monitor, PhysicalPosition, WebviewWindow};

/// Zones de travail de tous les écrans, et celle de l'écran principal.
pub struct Screens {
    /// Toutes les zones de travail.
    pub work_areas: Vec<Rect>,
    /// Zone de travail de l'écran principal (le premier écran à défaut).
    pub primary: Rect,
}

/// Les écrans branchés, `None` si le système n'en rend aucun.
pub fn screens(window: &WebviewWindow) -> Option<Screens> {
    let monitors = window.available_monitors().ok()?;
    let work_areas: Vec<Rect> = monitors.iter().map(logical_work_area).collect();
    let primary = window
        .primary_monitor()
        .ok()
        .flatten()
        .map(|monitor| logical_work_area(&monitor))
        .or_else(|| work_areas.first().copied())?;

    Some(Screens {
        work_areas,
        primary,
    })
}

fn logical_work_area(monitor: &Monitor) -> Rect {
    let area = monitor.work_area();
    let scale = monitor.scale_factor();

    Rect {
        x: to_logical_i32(f64::from(area.position.x), scale),
        y: to_logical_i32(f64::from(area.position.y), scale),
        width: to_logical_u32(f64::from(area.size.width), scale),
        height: to_logical_u32(f64::from(area.size.height), scale),
    }
}

/// Position logique d'une position physique de la fenêtre.
pub fn logical_position(window: &WebviewWindow, physical: PhysicalPosition<i32>) -> Position {
    let scale = window.scale_factor().unwrap_or(1.0);
    Position {
        x: to_logical_i32(f64::from(physical.x), scale),
        y: to_logical_i32(f64::from(physical.y), scale),
    }
}

/// Position logique actuelle de la fenêtre.
pub fn current_position(window: &WebviewWindow) -> Option<Position> {
    window
        .outer_position()
        .ok()
        .map(|physical| logical_position(window, physical))
}

fn to_logical_i32(value: f64, scale: f64) -> i32 {
    let logical = (value / scale).round();
    // Coordonnées d'écran : toujours dans les bornes d'i32
    logical.clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
}

fn to_logical_u32(value: f64, scale: f64) -> u32 {
    let logical = (value / scale).round();
    // Dimensions d'écran : toujours dans les bornes d'u32
    logical.clamp(0.0, f64::from(u32::MAX)) as u32
}
