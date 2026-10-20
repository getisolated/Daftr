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

/// Accélérateur à passer au plugin sur macOS. `global-hotkey` y enregistre un code de touche
/// physique (position QWERTY US) : la lettre `Q` vise la touche marquée « A » d'un AZERTY. La
/// touche physique enregistrée par le renderer (`KeyA`, nom que `global-hotkey` sait lire)
/// remplace donc la lettre finale. Seule une touche finale lettre ou chiffre est remplacée :
/// ce sont les seules dont le renderer enregistre la touche physique.
pub fn registration_accelerator(accelerator: &str, physical_key: Option<&str>) -> String {
    let Some(physical_key) = physical_key else {
        return accelerator.to_owned();
    };
    let (modifiers, key) = match accelerator.rsplit_once('+') {
        Some((modifiers, key)) => (Some(modifiers), key),
        None => (None, accelerator),
    };
    let mut chars = key.chars();
    let letter_or_digit = matches!(
        (chars.next(), chars.next()),
        (Some(c), None) if c.is_ascii_alphanumeric()
    );

    match modifiers {
        _ if !letter_or_digit => accelerator.to_owned(),
        Some(modifiers) => format!("{modifiers}+{physical_key}"),
        None => physical_key.to_owned(),
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
    fn keeps_the_accelerator_without_physical_key() {
        assert_eq!(
            registration_accelerator("CommandOrControl+Alt+Q", None),
            "CommandOrControl+Alt+Q"
        );
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
