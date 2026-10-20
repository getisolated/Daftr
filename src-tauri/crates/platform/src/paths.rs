//! Chemins canoniques.

use std::path::{Path, PathBuf};

/// Forme canonique d'un chemin : absolu, noms longs (pas la forme 8.3 `JOHNDO~1`, que les
/// surveillances de fichiers gèrent mal) et casse du disque, sans le préfixe `\\?\` que
/// Windows ajoute. Laissé absolu tel quel quand il n'existe pas (encore).
pub fn canonical_path(candidate: &Path) -> PathBuf {
    let absolute = std::path::absolute(candidate).unwrap_or_else(|_| candidate.to_path_buf());

    match std::fs::canonicalize(&absolute) {
        Ok(canonical) => strip_verbatim(canonical),
        Err(_) => absolute,
    }
}

/// `\\?\C:\x` → `C:\x` et `\\?\UNC\serveur\x` → `\\serveur\x`.
fn strip_verbatim(path: PathBuf) -> PathBuf {
    let text = path.to_string_lossy();

    if let Some(unc) = text.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{unc}"));
    }
    match text.strip_prefix(r"\\?\") {
        Some(local) => PathBuf::from(local),
        None => path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_the_verbatim_prefix() {
        assert_eq!(
            strip_verbatim(PathBuf::from(r"\\?\C:\x")),
            PathBuf::from(r"C:\x")
        );
        assert_eq!(
            strip_verbatim(PathBuf::from(r"\\?\UNC\srv\x")),
            PathBuf::from(r"\\srv\x")
        );
        assert_eq!(
            strip_verbatim(PathBuf::from("/tmp/x")),
            PathBuf::from("/tmp/x")
        );
    }

    #[test]
    fn keeps_a_missing_path_absolute() {
        let missing = Path::new("does-not-exist-anywhere.md");
        assert!(canonical_path(missing).is_absolute());
    }

    #[test]
    fn resolves_an_existing_path() {
        let current = std::env::current_dir().unwrap_or_default();
        let canonical = canonical_path(Path::new("."));
        assert!(canonical.is_absolute());
        assert!(!canonical.to_string_lossy().starts_with(r"\\?\"));
        assert_eq!(canonical_path(&current), canonical);
    }
}
