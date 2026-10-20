use std::sync::Arc;

use domain::i18n::Language;
use domain::overlay::Position;
use domain::settings::{AppSettings, RawPosition, RawSettings};
use platform::sync::lock;

use super::SettingsService;
use crate::testing::MemorySettings;

fn service(stored: Option<RawSettings>) -> (Arc<MemorySettings>, SettingsService) {
    let store = Arc::new(MemorySettings {
        stored: std::sync::Mutex::new(stored),
        ..MemorySettings::default()
    });
    (store.clone(), SettingsService::new(store))
}

#[test]
fn follows_the_os_locale_on_the_first_launch_and_remembers_it() {
    let (store, service) = service(None);

    assert_eq!(service.load(Some("fr-FR")).language, Language::Fr);
    assert_eq!(
        lock(&store.saved).last().map(|s| s.language),
        Some(Language::Fr)
    );
}

#[test]
fn falls_back_to_english_for_an_untranslated_locale() {
    let (_, service) = service(None);
    assert_eq!(service.load(Some("de-DE")).language, Language::En);
}

#[test]
fn picks_the_locale_for_settings_saved_before_the_language_existed() {
    let (_, service) = service(Some(RawSettings {
        word_wrap: Some(false),
        ..RawSettings::default()
    }));

    let settings = service.load(Some("fr"));

    assert_eq!(settings.language, Language::Fr);
    assert!(!settings.word_wrap);
}

#[test]
fn keeps_the_language_the_user_chose() {
    let (store, service) = service(Some(RawSettings {
        language: Some("en".into()),
        ..RawSettings::default()
    }));

    assert_eq!(service.load(Some("fr")).language, Language::En);
    assert!(lock(&store.saved).is_empty());
}

#[test]
fn a_patch_never_moves_the_overlay() {
    let (_, service) = service(None);
    service.set_overlay_position(Position { x: 5, y: 6 });

    let candidate = service.candidate(RawSettings {
        overlay_position: RawPosition::At { x: 1.0, y: 1.0 },
        word_wrap: Some(false),
        ..RawSettings::default()
    });

    assert_eq!(candidate.overlay_position, Some(Position { x: 5, y: 6 }));
    assert!(!candidate.word_wrap);
}

#[test]
fn a_candidate_is_not_applied_until_committed() {
    let (store, service) = service(None);

    let candidate = service.candidate(RawSettings {
        show_outline: Some(true),
        ..RawSettings::default()
    });
    assert!(!service.current().show_outline);

    let change = service.commit(candidate);
    assert!(service.current().show_outline);
    assert!(!change.before.show_outline && change.after.show_outline);
    assert_eq!(lock(&store.saved).len(), 1);
}

#[test]
fn reports_what_changed() {
    let (_, service) = service(None);
    let next = AppSettings {
        global_shortcut: "Alt+K".into(),
        overlay_width: 300,
        ..AppSettings::default()
    };

    let change = service.commit(next);

    assert!(change.shortcut_changed() && change.overlay_size_changed());
    assert!(!change.language_changed() && !change.collections_exclude_changed());
}

#[test]
fn reports_a_hidden_menu_bar_icon() {
    let (_, service) = service(None);
    let next = AppSettings {
        show_menu_bar_icon: false,
        ..AppSettings::default()
    };

    let change = service.commit(next);

    assert!(change.menu_bar_icon_changed());
    assert!(!change.launch_at_startup_changed() && !change.shortcut_changed());
}

#[test]
fn a_new_physical_key_is_a_shortcut_change() {
    let (_, service) = service(None);
    let next = AppSettings {
        global_shortcut_physical_key: Some("KeyA".into()),
        ..AppSettings::default()
    };

    let change = service.commit(next);

    assert!(change.shortcut_changed());
}
