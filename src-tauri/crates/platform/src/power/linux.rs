//! Batterie sous Linux : les sources d'alimentation de `sysfs`. Compilé partout pour que les
//! tests tournent sur chaque système.

#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::path::Path;

/// Dossier des sources d'alimentation du noyau.
pub(super) const POWER_SUPPLY_DIR: &str = "/sys/class/power_supply";

/// `None` si `directory` est illisible (pas de `sysfs`, conteneur) ; sinon, une batterie
/// système y figure ou non.
pub(super) fn battery_presence(directory: &Path) -> Option<bool> {
    let entries = std::fs::read_dir(directory).ok()?;

    Some(
        entries
            .flatten()
            .any(|entry| is_system_battery(&entry.path())),
    )
}

/// `BAT0`, `BAT1`… ou une source de type `Battery`, sauf celles d'un périphérique (souris,
/// casque sans fil : `scope` vaut `Device`).
fn is_system_battery(supply: &Path) -> bool {
    let named_battery = supply
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("BAT"));
    let typed_battery = read_trimmed(&supply.join("type")).as_deref() == Some("Battery");
    let peripheral = read_trimmed(&supply.join("scope")).as_deref() == Some("Device");

    (named_battery || typed_battery) && !peripheral
}

fn read_trimmed(path: &Path) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|text| text.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn supply(root: &Path, name: &str, kind: &str, scope: Option<&str>) {
        let directory = root.join(name);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("type"), format!("{kind}\n")).unwrap();
        if let Some(scope) = scope {
            std::fs::write(directory.join("scope"), scope).unwrap();
        }
    }

    #[test]
    fn a_system_battery_means_a_laptop() {
        let root = tempfile::tempdir().unwrap();
        supply(root.path(), "AC", "Mains", None);
        supply(root.path(), "BAT0", "Battery", None);
        assert_eq!(battery_presence(root.path()), Some(true));
    }

    #[test]
    fn a_battery_with_another_name_counts() {
        let root = tempfile::tempdir().unwrap();
        supply(root.path(), "CMB1", "Battery", Some("System"));
        assert_eq!(battery_presence(root.path()), Some(true));
    }

    #[test]
    fn peripheral_batteries_do_not_count() {
        let root = tempfile::tempdir().unwrap();
        supply(root.path(), "AC", "Mains", None);
        supply(root.path(), "hidpp_battery_0", "Battery", Some("Device"));
        assert_eq!(battery_presence(root.path()), Some(false));
    }

    #[test]
    fn an_unreadable_directory_is_unknown() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(battery_presence(&root.path().join("missing")), None);
    }
}
