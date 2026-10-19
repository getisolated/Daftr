//! Le matériau acrylique de Windows (flou de ce qui est derrière la fenêtre) est-il rendu ?
//!
//! Windows le remplace par un gris uni sous l'économiseur de batterie ou quand les effets de
//! transparence sont désactivés ; aucune API de Tauri ne l'expose, d'où une sonde PowerShell
//! (`GetSystemPowerStatus` et la clé `Personalize`), sans code `unsafe`.

use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use platform::sync::lock;

/// L'acrylique de fenêtre demande Windows 11 22H2.
const ACRYLIC_MIN_WINDOWS_BUILD: u32 = 22621;
const PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const POLL_INTERVAL: Duration = Duration::from_millis(50);

const PROBE_SCRIPT: &str = concat!(
    "Add-Type 'using System.Runtime.InteropServices; public class PowerProbe { ",
    "[StructLayout(LayoutKind.Sequential)] public struct Status { public byte Line; ",
    "public byte Flag; public byte Life; public byte Saver; public int A; public int B; } ",
    "[DllImport(\"kernel32.dll\")] public static extern bool GetSystemPowerStatus(out Status s); }';",
    "$status = New-Object PowerProbe+Status;",
    "[void][PowerProbe]::GetSystemPowerStatus([ref]$status);",
    "$key = 'HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize';",
    "$transparency = (Get-ItemProperty $key -ErrorAction SilentlyContinue).EnableTransparency;",
    "\"$($status.Saver),$transparency,$([Environment]::OSVersion.Version.Build)\"",
);

/// Dernier état connu (`None` : jamais sondé).
static ACTIVE: Mutex<Option<bool>> = Mutex::new(None);

/// L'acrylique est-il rendu ? Le premier appel paie la sonde (jusqu'à 5 s), les suivants
/// sont immédiats.
pub fn is_active() -> bool {
    if let Some(active) = *lock(&ACTIVE) {
        return active;
    }
    let active = probe();
    *lock(&ACTIVE) = Some(active);
    active
}

/// Sonde à nouveau en arrière-plan : le prochain appel voit un changement d'économiseur de
/// batterie ou de transparence.
pub fn refresh_in_background() {
    let spawned = std::thread::Builder::new()
        .name("acrylic-probe".into())
        .spawn(|| {
            let active = probe();
            *lock(&ACTIVE) = Some(active);
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "sonde de l'acrylique non lancée");
    }
}

fn probe() -> bool {
    match run_probe() {
        Some(output) => parse_probe(&output),
        // État inconnu : on suppose l'effet rendu, le comportement normal d'un Windows 11
        None => true,
    }
}

fn run_probe() -> Option<String> {
    let mut child = probe_command()
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let started = Instant::now();

    while child.try_wait().ok()?.is_none() {
        if started.elapsed() > PROBE_TIMEOUT {
            // Sonde bloquée : elle est abandonnée, son échec éventuel n'importe plus
            let _ = child.kill();
            return None;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    let output = child.wait_with_output().ok()?;
    String::from_utf8(output.stdout).ok()
}

fn probe_command() -> Command {
    let mut command = Command::new("powershell.exe");
    command.args(["-NoProfile", "-NonInteractive", "-Command", PROBE_SCRIPT]);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

/// `économiseur,transparence,build` → acrylique rendu : Windows 11 22H2+, économiseur coupé
/// (`1` = actif) et transparence non désactivée (`0`).
fn parse_probe(output: &str) -> bool {
    let fields: Vec<&str> = output.trim().split(',').collect();
    let [saver, transparency, build] = fields.as_slice() else {
        return true;
    };
    let recent_windows = build
        .trim()
        .parse::<u32>()
        .is_ok_and(|b| b >= ACRYLIC_MIN_WINDOWS_BUILD);

    recent_windows && *saver != "1" && *transparency != "0"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acrylic_needs_a_recent_windows_without_battery_saver_or_disabled_transparency() {
        assert!(parse_probe("0,1,26100\r\n"));
        assert!(parse_probe("0,,22631"));
        assert!(!parse_probe("1,1,26100"));
        assert!(!parse_probe("0,0,26100"));
        assert!(!parse_probe("0,1,19045"));
    }

    #[test]
    fn an_unreadable_probe_assumes_acrylic() {
        assert!(parse_probe("garbage"));
    }
}
