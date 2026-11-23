//! Batterie sous Windows : `GetSystemPowerStatus`, seul appel `unsafe` du crate (FFI).

use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

/// `BatteryFlag` : aucune batterie système.
const NO_SYSTEM_BATTERY: u8 = 128;
/// `BatteryFlag` : état de la batterie inconnu.
const UNKNOWN_STATUS: u8 = 255;

/// `None` si l'appel échoue ou si Windows ne connaît pas l'état de la batterie.
pub(super) fn battery_presence() -> Option<bool> {
    let mut status = SYSTEM_POWER_STATUS::default();
    // SAFETY: `status` est une structure `repr(C)` possédée, valide et modifiable pendant
    // tout l'appel ; la fonction la remplit et ne garde pas le pointeur.
    let succeeded = unsafe { GetSystemPowerStatus(&mut status) } != 0;

    succeeded.then_some(status.BatteryFlag).and_then(from_flag)
}

/// Lecture de `BatteryFlag` (champ de bits).
fn from_flag(flag: u8) -> Option<bool> {
    if flag == UNKNOWN_STATUS {
        return None;
    }

    Some(flag & NO_SYSTEM_BATTERY == 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battery_flags_are_read() {
        assert_eq!(from_flag(NO_SYSTEM_BATTERY), Some(false));
        assert_eq!(from_flag(UNKNOWN_STATUS), None);
        // Haut (1), charge (8)
        assert_eq!(from_flag(1 | 8), Some(true));
        assert_eq!(from_flag(0), Some(true));
    }

    #[test]
    fn the_system_answers_without_failing() {
        // Le résultat dépend de la machine : seul l'appel lui-même est vérifié
        let _ = battery_presence();
    }
}
