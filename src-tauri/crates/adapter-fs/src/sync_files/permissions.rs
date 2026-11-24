//! Écriture d'un fichier lisible par l'utilisateur seul (clé privée de l'appareil).
//!
//! Unix : mode `0600` dès la création. Windows : les ACL héritées sont retirées et seul
//! l'utilisateur courant garde l'accès (`icacls`, sans code natif) ; en cas d'échec, le
//! fichier garde les ACL du dossier de données (`%APPDATA%`, déjà propre à l'utilisateur).

use std::io;
use std::path::Path;

/// Écrit `bytes` dans un nouveau fichier `path` (remplacé s'il existe) accessible à
/// l'utilisateur seul.
pub(super) fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    remove_if_exists(path)?;
    write_new(path, bytes)?;
    restrict(path);
    Ok(())
}

fn remove_if_exists(path: &Path) -> io::Result<()> {
    match std::fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        Ok(()) | Err(_) => Ok(()),
    }
}

#[cfg(unix)]
fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(not(unix))]
fn write_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    std::fs::write(path, bytes)
}

#[cfg(unix)]
fn restrict(_path: &Path) {}

/// `icacls <fichier> /inheritance:r /grant:r <domaine>\<utilisateur>:F` : un échec (nom
/// introuvable, outil absent) est journalisé, le fichier reste protégé par son dossier.
#[cfg(windows)]
fn restrict(path: &Path) {
    use std::os::windows::process::CommandExt;

    /// Pas de console qui clignote derrière l'application.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    let Some(user) = current_user() else {
        return tracing::warn!("utilisateur courant inconnu : ACL de device.key héritées");
    };
    let status = std::process::Command::new("icacls")
        .arg(path)
        .args(["/inheritance:r", "/grant:r"])
        .arg(format!("{user}:F"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status();
    match status {
        Ok(status) if status.success() => {}
        Ok(status) => tracing::warn!(%status, "icacls a échoué : ACL de device.key héritées"),
        Err(error) => tracing::warn!(%error, "icacls indisponible : ACL de device.key héritées"),
    }
}

#[cfg(windows)]
fn current_user() -> Option<String> {
    let user = std::env::var("USERNAME")
        .ok()
        .filter(|user| !user.is_empty())?;
    Some(match std::env::var("USERDOMAIN") {
        Ok(domain) if !domain.is_empty() => format!("{domain}\\{user}"),
        _ => user,
    })
}

#[cfg(not(any(unix, windows)))]
fn restrict(_path: &Path) {}
