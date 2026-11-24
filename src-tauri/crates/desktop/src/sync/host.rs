//! Nom de l'appareil par défaut : le nom d'hôte du système, sans dépendance ni code natif.

use domain::i18n::{Language, MessageKey, message};
use domain::sync::DeviceName;

/// Le nom d'hôte rendu valide, sinon un nom générique traduit.
pub fn default_device_name(language: Language) -> Option<DeviceName> {
    system_name()
        .and_then(|name| DeviceName::sanitize(&name))
        .or_else(|| DeviceName::sanitize(message(language, MessageKey::SyncDefaultDeviceName)))
}

/// `COMPUTERNAME` (nom NetBIOS, sans appel système).
#[cfg(windows)]
fn system_name() -> Option<String> {
    std::env::var("COMPUTERNAME").ok()
}

/// Le nom de l'ordinateur des Réglages, sinon le nom d'hôte.
#[cfg(target_os = "macos")]
fn system_name() -> Option<String> {
    command_output("scutil", &["--get", "ComputerName"]).or_else(|| command_output("hostname", &[]))
}

/// `/etc/hostname`, sinon la commande `hostname`.
#[cfg(all(unix, not(target_os = "macos")))]
fn system_name() -> Option<String> {
    std::fs::read_to_string("/etc/hostname")
        .ok()
        .filter(|name| !name.trim().is_empty())
        .or_else(|| command_output("hostname", &[]))
}

#[cfg(not(any(unix, windows)))]
fn system_name() -> Option<String> {
    None
}

#[cfg(unix)]
fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(program)
        .args(args)
        .output()
        .ok()?;
    let text = String::from_utf8(output.stdout).ok()?;
    (output.status.success() && !text.trim().is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn always_gives_a_valid_name() {
        assert!(default_device_name(Language::En).is_some());
        assert!(
            DeviceName::parse(message(Language::Fr, MessageKey::SyncDefaultDeviceName)).is_some()
        );
    }
}
