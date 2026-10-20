//! Raccourci global (par défaut `Ctrl+Alt+Q`).

use std::sync::Mutex;

use domain::settings::ShortcutMode;
use platform::sync::lock;
use tauri::AppHandle;
use tauri_plugin_global_shortcut::{
    GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState as KeyState,
};

use crate::state::desktop;
use crate::windows::{main_window, overlay};

/// Accélérateur enregistré (tel que passé au plugin : avec la touche physique sur macOS).
#[derive(Default)]
pub struct ShortcutState {
    registered: Mutex<Option<String>>,
}

/// Fait de `accelerator` l'unique raccourci global. En cas d'échec (invalide, déjà pris par
/// une autre application) le précédent est gardé : l'utilisateur n'en reste jamais privé.
/// `physical_key` (`KeyboardEvent.code` de la touche finale) ne sert que sur macOS.
pub fn register(app: &AppHandle, accelerator: &str, physical_key: Option<&str>) -> bool {
    let accelerator = if cfg!(target_os = "macos") {
        registration_accelerator(accelerator, physical_key)
    } else {
        accelerator.to_owned()
    };
    let accelerator = accelerator.as_str();
    let state = desktop(app);
    let mut registered = lock(&state.shortcut.registered);
    if registered.as_deref() == Some(accelerator) {
        return true;
    }
    let shortcuts = app.global_shortcut();

    if let Err(error) = shortcuts.on_shortcut(accelerator, on_shortcut) {
        tracing::warn!(%error, accelerator, "raccourci global refusé");
        return false;
    }
    if let Some(previous) = registered.replace(accelerator.to_owned())
        && let Err(error) = shortcuts.unregister(previous.as_str())
    {
        tracing::warn!(%error, previous, "ancien raccourci non libéré");
    }
    true
}

/// Code de touche physique (nom lu par `global-hotkey`) de la touche portant `key` sur un
/// clavier AZERTY, ou `None` si `key` n'est ni une lettre ASCII ni un chiffre. Seules les
/// lettres dont la position diffère du QWERTY US sont permutées : `A`/`Q`, `Z`/`W`, et `M`
/// qui occupe la position du `;` QWERTY. Les chiffres gardent leur position.
fn azerty_physical_key(key: char) -> Option<String> {
    if key.is_ascii_digit() {
        return Some(format!("Digit{key}"));
    }
    if !key.is_ascii_alphabetic() {
        return None;
    }
    let letter = key.to_ascii_uppercase();
    Some(match letter {
        'A' => "KeyQ".to_owned(),
        'Q' => "KeyA".to_owned(),
        'Z' => "KeyW".to_owned(),
        'W' => "KeyZ".to_owned(),
        'M' => "Semicolon".to_owned(),
        _ => format!("Key{letter}"),
    })
}

/// Accélérateur à passer au plugin sur macOS. `global-hotkey` y enregistre un code de touche
/// physique (position QWERTY US) : la lettre `Q` vise la touche marquée « A » d'un AZERTY. La
/// touche physique enregistrée par le renderer (`KeyA`, nom que `global-hotkey` sait lire)
/// remplace donc la lettre finale. Seule une touche finale lettre ou chiffre est remplacée :
/// ce sont les seules dont le renderer enregistre la touche physique.
///
/// Repli provisoire : sans touche physique connue (réglage par défaut, remise au défaut,
/// anciens réglages), un clavier AZERTY est supposé (`azerty_physical_key`), en attendant une
/// détection de la disposition réelle.
pub fn registration_accelerator(accelerator: &str, physical_key: Option<&str>) -> String {
    let (modifiers, key) = match accelerator.rsplit_once('+') {
        Some((modifiers, key)) => (Some(modifiers), key),
        None => (None, accelerator),
    };
    let mut chars = key.chars();
    let single = match (chars.next(), chars.next()) {
        (Some(c), None) if c.is_ascii_alphanumeric() => Some(c),
        _ => None,
    };
    let Some(single) = single else {
        return accelerator.to_owned();
    };
    let Some(physical_key) = physical_key
        .map(str::to_owned)
        .or_else(|| azerty_physical_key(single))
    else {
        return accelerator.to_owned();
    };

    match modifiers {
        Some(modifiers) => format!("{modifiers}+{physical_key}"),
        None => physical_key,
    }
}

fn on_shortcut(app: &AppHandle, _shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state != KeyState::Pressed {
        return;
    }
    match desktop(app).services.settings.current().shortcut_mode {
        ShortcutMode::Overlay => overlay::toggle(app),
        ShortcutMode::App => {
            overlay::hide(app);
            main_window::show(app);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::registration_accelerator;

    #[test]
    fn replaces_the_letter_with_the_physical_key() {
        assert_eq!(
            registration_accelerator("CommandOrControl+Alt+Q", Some("KeyA")),
            "CommandOrControl+Alt+KeyA"
        );
        assert_eq!(
            registration_accelerator("Control+Shift+1", Some("Digit1")),
            "Control+Shift+Digit1"
        );
        assert_eq!(registration_accelerator("Q", Some("KeyA")), "KeyA");
    }

    #[test]
    fn falls_back_to_azerty_without_physical_key() {
        let cases = [
            ("CommandOrControl+Alt+Q", "CommandOrControl+Alt+KeyA"),
            ("CommandOrControl+Alt+A", "CommandOrControl+Alt+KeyQ"),
            ("CommandOrControl+Alt+Z", "CommandOrControl+Alt+KeyW"),
            ("CommandOrControl+Alt+W", "CommandOrControl+Alt+KeyZ"),
            ("CommandOrControl+Alt+M", "CommandOrControl+Alt+Semicolon"),
            ("CommandOrControl+Alt+L", "CommandOrControl+Alt+KeyL"),
            ("CommandOrControl+Alt+1", "CommandOrControl+Alt+Digit1"),
            ("Q", "KeyA"),
        ];
        for (accelerator, expected) in cases {
            assert_eq!(registration_accelerator(accelerator, None), expected);
        }
    }

    #[test]
    fn explicit_physical_key_wins_over_the_azerty_fallback() {
        assert_eq!(
            registration_accelerator("CommandOrControl+Alt+Q", Some("KeyQ")),
            "CommandOrControl+Alt+KeyQ"
        );
    }

    #[test]
    fn keeps_a_non_letter_final_key_without_physical_key() {
        assert_eq!(registration_accelerator("Alt+F5", None), "Alt+F5");
    }

    #[test]
    fn keeps_a_final_key_that_is_not_a_letter_or_a_digit() {
        assert_eq!(
            registration_accelerator("CommandOrControl+Alt+F5", Some("KeyA")),
            "CommandOrControl+Alt+F5"
        );
        assert_eq!(
            registration_accelerator("CommandOrControl+Space", Some("KeyA")),
            "CommandOrControl+Space"
        );
        assert_eq!(registration_accelerator("Alt+,", Some("KeyM")), "Alt+,");
    }
}
