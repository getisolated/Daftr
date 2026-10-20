//! Placement de la fenêtre de note rapide (overlay) sur les écrans.
//!
//! Toutes les coordonnées sont logiques (indépendantes de la mise à l'échelle de l'écran),
//! celles sauvegardées dans `settings.json`.

/// Marge entre l'overlay et le bord de la zone de travail, à sa place par défaut.
pub const OVERLAY_MARGIN: i32 = 16;

/// Part minimale de l'overlay (par axe) qui doit rester sur un écran pour réutiliser une
/// position sauvegardée.
const MIN_VISIBLE: i64 = 80;

/// Coin haut-gauche d'une fenêtre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// Abscisse.
    pub x: i32,
    /// Ordonnée.
    pub y: i32,
}

/// Taille d'une fenêtre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Size {
    /// Largeur.
    pub width: u32,
    /// Hauteur.
    pub height: u32,
}

/// Zone de travail d'un écran (hors barre des tâches).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    /// Abscisse de l'origine.
    pub x: i32,
    /// Ordonnée de l'origine.
    pub y: i32,
    /// Largeur.
    pub width: u32,
    /// Hauteur.
    pub height: u32,
}

/// Place par défaut : le coin bas-droit de la zone de travail, avec une marge.
pub fn bottom_right_position(work_area: Rect, size: Size) -> Position {
    Position {
        x: far_edge(work_area.x, work_area.width, size.width),
        y: far_edge(work_area.y, work_area.height, size.height),
    }
}

fn far_edge(start: i32, area_length: u32, window_length: u32) -> i32 {
    let edge = i64::from(start) + i64::from(area_length) - i64::from(window_length);
    saturate(edge - i64::from(OVERLAY_MARGIN))
}

/// Où afficher l'overlay : la position sauvegardée si assez de la fenêtre reste visible sur
/// un écran branché (un écran a pu être débranché), sinon le coin bas-droit de l'écran
/// principal.
pub fn resolve_overlay_position(
    saved: Option<Position>,
    size: Size,
    work_areas: &[Rect],
    primary_work_area: Rect,
) -> Position {
    match saved {
        Some(position) if is_visible_on_any(position, size, work_areas) => position,
        Some(_) | None => bottom_right_position(primary_work_area, size),
    }
}

fn is_visible_on_any(position: Position, size: Size, work_areas: &[Rect]) -> bool {
    let needed_x = MIN_VISIBLE.min(i64::from(size.width));
    let needed_y = MIN_VISIBLE.min(i64::from(size.height));

    work_areas.iter().any(|area| {
        let visible_x = overlap(position.x, size.width, area.x, area.width);
        let visible_y = overlap(position.y, size.height, area.y, area.height);
        visible_x >= needed_x && visible_y >= needed_y
    })
}

fn overlap(start_a: i32, length_a: u32, start_b: i32, length_b: u32) -> i64 {
    let start = i64::from(start_a).max(i64::from(start_b));
    let end_a = i64::from(start_a) + i64::from(length_a);
    let end_b = i64::from(start_b) + i64::from(length_b);

    (end_a.min(end_b) - start).max(0)
}

/// Nouveau coin haut-gauche après un redimensionnement, le coin bas-droit restant fixe (le
/// champ de saisie est en bas).
pub fn anchor_bottom_right(position: Position, old_size: Size, new_size: Size) -> Position {
    let shift_x = i64::from(old_size.width) - i64::from(new_size.width);
    let shift_y = i64::from(old_size.height) - i64::from(new_size.height);

    Position {
        x: saturate(i64::from(position.x) + shift_x),
        y: saturate(i64::from(position.y) + shift_y),
    }
}

fn saturate(value: i64) -> i32 {
    i32::try_from(value).unwrap_or(if value < 0 { i32::MIN } else { i32::MAX })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN_1: Rect = Rect {
        x: 0,
        y: 0,
        width: 1920,
        height: 1040,
    };
    const SCREEN_2: Rect = Rect {
        x: 1920,
        y: 0,
        width: 1280,
        height: 1024,
    };
    const SIZE: Size = Size {
        width: 250,
        height: 500,
    };

    fn at(x: i32, y: i32) -> Position {
        Position { x, y }
    }

    #[test]
    fn places_the_overlay_bottom_right_with_a_margin() {
        assert_eq!(
            bottom_right_position(SCREEN_1, SIZE),
            at(1920 - 250 - 16, 1040 - 500 - 16)
        );
    }

    #[test]
    fn respects_the_origin_of_a_secondary_monitor() {
        let expected = at(1920 + 1280 - 250 - 16, 1024 - 500 - 16);
        assert_eq!(bottom_right_position(SCREEN_2, SIZE), expected);
    }

    #[test]
    fn defaults_to_bottom_right_without_saved_position() {
        let resolved = resolve_overlay_position(None, SIZE, &[SCREEN_1], SCREEN_1);
        assert_eq!(resolved, bottom_right_position(SCREEN_1, SIZE));
    }

    #[test]
    fn reuses_a_visible_saved_position() {
        let resolved = resolve_overlay_position(Some(at(300, 200)), SIZE, &[SCREEN_1], SCREEN_1);
        assert_eq!(resolved, at(300, 200));
    }

    #[test]
    fn reuses_a_saved_position_on_a_secondary_monitor() {
        let screens = [SCREEN_1, SCREEN_2];
        let resolved = resolve_overlay_position(Some(at(2000, 100)), SIZE, &screens, SCREEN_1);
        assert_eq!(resolved, at(2000, 100));
    }

    #[test]
    fn falls_back_when_the_saved_monitor_is_gone() {
        let resolved = resolve_overlay_position(Some(at(2000, 100)), SIZE, &[SCREEN_1], SCREEN_1);
        assert_eq!(resolved, bottom_right_position(SCREEN_1, SIZE));
    }

    #[test]
    fn falls_back_when_the_window_is_almost_off_screen() {
        let resolved = resolve_overlay_position(Some(at(1900, 100)), SIZE, &[SCREEN_1], SCREEN_1);
        assert_eq!(resolved, bottom_right_position(SCREEN_1, SIZE));
    }

    #[test]
    fn keeps_the_bottom_right_corner_when_growing() {
        let grown = Size {
            width: 300,
            height: 600,
        };
        assert_eq!(anchor_bottom_right(at(100, 100), SIZE, grown), at(50, 0));
    }

    #[test]
    fn keeps_the_bottom_right_corner_when_shrinking() {
        let shrunk = Size {
            width: 200,
            height: 400,
        };
        assert_eq!(
            anchor_bottom_right(at(100, 100), SIZE, shrunk),
            at(150, 200)
        );
    }
}
