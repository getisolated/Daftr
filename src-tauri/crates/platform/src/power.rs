//! Présence d'une batterie système, lue auprès du système d'exploitation : distingue un
//! ordinateur portable d'un ordinateur fixe.

mod linux;
mod macos;
#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;

/// `Some(true)` si l'appareil a une batterie système, `Some(false)` s'il n'en a pas, `None`
/// quand le système ne permet pas de le savoir (détection en échec, système non géré).
///
/// - Windows : `GetSystemPowerStatus` (`BatteryFlag` 128 : aucune batterie système).
/// - Linux : une entrée `BAT*` ou de type `Battery` dans `/sys/class/power_supply`.
/// - macOS : une `InternalBattery` dans `pmset -g batt`.
pub fn battery_presence() -> Option<bool> {
    probe()
}

#[cfg(windows)]
fn probe() -> Option<bool> {
    windows::battery_presence()
}

#[cfg(target_os = "linux")]
fn probe() -> Option<bool> {
    linux::battery_presence(std::path::Path::new(linux::POWER_SUPPLY_DIR))
}

#[cfg(target_os = "macos")]
fn probe() -> Option<bool> {
    macos::battery_presence()
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
fn probe() -> Option<bool> {
    None
}
