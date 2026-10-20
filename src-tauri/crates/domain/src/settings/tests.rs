use super::*;

fn sanitize(raw: RawSettings) -> AppSettings {
    sanitize_settings(&raw, &AppSettings::default())
}

fn text(value: &str) -> Option<String> {
    Some(value.to_owned())
}

#[test]
fn returns_the_base_for_an_empty_input() {
    assert_eq!(sanitize(RawSettings::default()), AppSettings::default());
}

#[test]
fn keeps_valid_values() {
    let result = sanitize(RawSettings {
        global_shortcut: text("Ctrl+Shift+K"),
        shortcut_mode: text("app"),
        overlay_width: Some(320.0),
        overlay_height: Some(640.0),
        enter_to_send: Some(false),
        ..RawSettings::default()
    });

    assert_eq!(result.global_shortcut, "Ctrl+Shift+K");
    assert_eq!(result.shortcut_mode, ShortcutMode::App);
    assert_eq!(
        result.overlay_size(),
        crate::overlay::Size {
            width: 320,
            height: 640
        }
    );
    assert!(!result.enter_to_send);
}

#[test]
fn keeps_valid_formatting_bar_preferences() {
    let result = sanitize(RawSettings {
        formatting_bar_enabled: Some(false),
        formatting_bar_mode: text("compact"),
        formatting_bar_style: text("pinned"),
        formatting_bar_position: text("left"),
        ..RawSettings::default()
    });

    assert!(!result.formatting_bar_enabled);
    assert_eq!(result.formatting_bar_mode, FormattingBarMode::Compact);
    assert_eq!(result.formatting_bar_style, FormattingBarStyle::Pinned);
    assert_eq!(result.formatting_bar_position, FormattingBarPosition::Left);
}

#[test]
fn falls_back_on_invalid_formatting_bar_preferences() {
    let result = sanitize(RawSettings {
        formatting_bar_mode: text("huge"),
        formatting_bar_position: text("middle"),
        ..RawSettings::default()
    });

    assert_eq!(result.formatting_bar_mode, FormattingBarMode::Full);
    assert_eq!(result.formatting_bar_style, FormattingBarStyle::Floating);
    assert_eq!(result.formatting_bar_position, FormattingBarPosition::Top);
}

#[test]
fn keeps_a_valid_theme_and_falls_back_to_the_current_one() {
    let theme = |value: &str, base: &AppSettings| {
        let raw = RawSettings {
            theme: text(value),
            ..RawSettings::default()
        };
        sanitize_settings(&raw, base).theme
    };
    let defaults = AppSettings::default();
    let light = AppSettings {
        theme: ThemePreference::Theme(ThemeId::Light),
        ..defaults.clone()
    };

    assert_eq!(
        theme("light", &defaults),
        ThemePreference::Theme(ThemeId::Light)
    );
    assert_eq!(theme("system", &defaults), ThemePreference::System);
    assert_eq!(
        theme("github-dark-dimmed", &defaults),
        ThemePreference::Theme(ThemeId::GithubDarkDimmed)
    );
    assert_eq!(
        theme("solarized", &defaults),
        ThemePreference::Theme(ThemeId::Dark)
    );
    assert_eq!(
        theme("nope", &light),
        ThemePreference::Theme(ThemeId::Light)
    );
}

#[test]
fn keeps_system_themes_only_of_the_matching_family() {
    let valid = sanitize(RawSettings {
        preferred_dark_theme: text("github-dark-dimmed"),
        preferred_light_theme: text("solarized-light"),
        ..RawSettings::default()
    });
    let swapped = sanitize(RawSettings {
        preferred_dark_theme: text("violet-light"),
        preferred_light_theme: text("dark"),
        ..RawSettings::default()
    });

    assert_eq!(valid.preferred_dark_theme, ThemeId::GithubDarkDimmed);
    assert_eq!(valid.preferred_light_theme, ThemeId::SolarizedLight);
    assert_eq!(swapped.preferred_dark_theme, ThemeId::Dark);
    assert_eq!(swapped.preferred_light_theme, ThemeId::Light);
}

#[test]
fn keeps_a_clean_list_of_excluded_folders() {
    let items = [" build ", "build", "", "a/b", "c\\d", "tmp"].map(text);
    let mut with_number = items.to_vec();
    with_number.insert(4, None);

    let result = sanitize(RawSettings {
        collections_exclude: Some(with_number),
        ..RawSettings::default()
    });

    assert_eq!(result.collections_exclude, ["build", "tmp"]);
}

#[test]
fn keeps_the_current_excludes_when_the_value_is_not_a_list() {
    let result = sanitize(RawSettings {
        collections_exclude: None,
        ..RawSettings::default()
    });
    assert_eq!(
        result.collections_exclude,
        AppSettings::default().collections_exclude
    );
}

#[test]
fn clamps_the_overlay_size() {
    let result = sanitize(RawSettings {
        overlay_width: Some(10.0),
        overlay_height: Some(99999.0),
        ..RawSettings::default()
    });

    assert_eq!((result.overlay_width, result.overlay_height), (200, 1200));
}

#[test]
fn rejects_non_finite_sizes() {
    let result = sanitize(RawSettings {
        overlay_width: Some(f64::NAN),
        overlay_height: Some(f64::INFINITY),
        ..RawSettings::default()
    });

    assert_eq!((result.overlay_width, result.overlay_height), (250, 350));
}

#[test]
fn keeps_valid_app_shortcuts_and_drops_malformed_ones() {
    let entries = vec![
        ("note.new".to_owned(), text("CommandOrControl+Alt+M")),
        ("Bad Id!".to_owned(), text("Ctrl+X")),
        ("tab.close".to_owned(), text("")),
        ("tab.pin".to_owned(), None),
    ];
    let result = sanitize(RawSettings {
        app_shortcuts: Some(entries),
        ..RawSettings::default()
    });

    assert_eq!(
        result.app_shortcuts,
        [("note.new".to_owned(), "CommandOrControl+Alt+M".to_owned())]
    );
}

#[test]
fn keeps_the_current_app_shortcuts_when_the_value_is_not_an_object() {
    let shortcuts = vec![("note.new".to_owned(), "Alt+M".to_owned())];
    let base = AppSettings {
        app_shortcuts: shortcuts.clone(),
        ..AppSettings::default()
    };

    let result = sanitize_settings(&RawSettings::default(), &base);

    assert_eq!(result.app_shortcuts, shortcuts);
}

#[test]
fn rejects_an_unknown_mode_and_an_empty_shortcut() {
    let result = sanitize(RawSettings {
        shortcut_mode: text("weird"),
        global_shortcut: text("  "),
        ..RawSettings::default()
    });

    assert_eq!(result.shortcut_mode, ShortcutMode::Overlay);
    assert_eq!(
        result.global_shortcut,
        AppSettings::default().global_shortcut
    );
}

#[test]
fn falls_back_to_the_current_values_rather_than_the_defaults() {
    let base = AppSettings {
        overlay_width: 400,
        ..AppSettings::default()
    };
    let raw = RawSettings {
        overlay_width: None,
        ..RawSettings::default()
    };

    assert_eq!(sanitize_settings(&raw, &base).overlay_width, 400);
}

#[test]
fn rounds_the_saved_position_and_drops_an_invalid_one() {
    let rounded = sanitize(RawSettings {
        overlay_position: RawPosition::At { x: 10.4, y: 20.6 },
        ..RawSettings::default()
    });
    let invalid = sanitize(RawSettings {
        overlay_position: RawPosition::Invalid,
        ..RawSettings::default()
    });

    assert_eq!(
        rounded.overlay_position,
        Some(crate::overlay::Position { x: 10, y: 21 })
    );
    assert_eq!(invalid.overlay_position, None);
}

#[test]
fn keeps_the_position_when_absent() {
    let position = Some(crate::overlay::Position { x: -5, y: 3 });
    let base = AppSettings {
        overlay_position: position,
        ..AppSettings::default()
    };

    assert_eq!(
        sanitize_settings(&RawSettings::default(), &base).overlay_position,
        position
    );
}

#[test]
fn round_trips_valid_settings() {
    let settings = AppSettings {
        theme: ThemePreference::System,
        app_shortcuts: vec![("tab.close".to_owned(), "Ctrl+W".to_owned())],
        overlay_position: Some(crate::overlay::Position { x: 1, y: 2 }),
        ..AppSettings::default()
    };

    let raw = RawSettings::from_settings(&settings);

    assert_eq!(sanitize_settings(&raw, &AppSettings::default()), settings);
}

#[test]
fn a_patch_overrides_only_its_fields() {
    let base = RawSettings::from_settings(&AppSettings::default());
    let patch = RawSettings {
        word_wrap: Some(false),
        ..RawSettings::default()
    };

    let merged = sanitize(base.overridden_by(patch));

    assert!(!merged.word_wrap);
    assert_eq!(merged.overlay_width, AppSettings::default().overlay_width);
}

#[test]
fn every_theme_has_a_family_and_a_background() {
    for theme in ThemeId::ALL {
        let resolved = ThemePreference::Theme(*theme).resolve(true, ThemeId::Dark, ThemeId::Light);
        assert_eq!(resolved, *theme);
        assert_ne!(theme.background_rgb(), (0, 0, 0));
    }
}

#[test]
fn system_follows_the_os_mode() {
    let system = ThemePreference::System;
    assert_eq!(
        system.resolve(true, ThemeId::GithubDarkDimmed, ThemeId::Light),
        ThemeId::GithubDarkDimmed
    );
    assert_eq!(
        system.resolve(false, ThemeId::Dark, ThemeId::SolarizedLight),
        ThemeId::SolarizedLight
    );
}
