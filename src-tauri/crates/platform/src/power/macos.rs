//! Batterie sous macOS : `pmset -g batt`. Le modèle (`hw.model`) ne suffit pas : les Mac
//! Apple Silicon portables s'appellent `MacXX,Y` comme les fixes. La lecture de la sortie est
//! compilée partout pour que ses tests tournent sur chaque système.

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

/// Chemin absolu : pas de recherche dans le `PATH`.
#[cfg(target_os = "macos")]
const PMSET: &str = "/usr/bin/pmset";

/// `None` si `pmset` échoue.
#[cfg(target_os = "macos")]
pub(super) fn battery_presence() -> Option<bool> {
    let output = std::process::Command::new(PMSET)
        .args(["-g", "batt"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    battery_in_pmset(&String::from_utf8_lossy(&output.stdout))
}

/// Une `InternalBattery` dans la sortie de `pmset -g batt` ; `None` pour une sortie vide.
pub(super) fn battery_in_pmset(output: &str) -> Option<bool> {
    if output.trim().is_empty() {
        return None;
    }

    Some(output.contains("InternalBattery"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_internal_battery_means_a_laptop() {
        let laptop = "Now drawing from 'AC Power'\n -InternalBattery-0 (id=123)\t100%; charged;";
        assert_eq!(battery_in_pmset(laptop), Some(true));
    }

    #[test]
    fn no_battery_means_a_desktop() {
        assert_eq!(
            battery_in_pmset("Now drawing from 'AC Power'\n"),
            Some(false)
        );
    }

    #[test]
    fn an_empty_output_is_unknown() {
        assert_eq!(battery_in_pmset("  \n"), None);
    }
}
