//! Chemins à l'intérieur d'une collection : le renderer ne désigne jamais un fichier
//! autrement que par un chemin relatif vérifié ici.

use std::path::{Component, Path, PathBuf};

/// Chemin absolu de `relative` dans `root`, ou `None` s'il en sortirait (`..`, chemin
/// absolu, autre lecteur) ou s'il désigne la racine elle-même.
///
/// ```
/// use std::path::Path;
/// use domain::collection::resolve_inside;
///
/// let root = Path::new("/repo");
/// assert_eq!(resolve_inside(root, "docs/a.md"), Some(root.join("docs").join("a.md")));
/// assert_eq!(resolve_inside(root, "../x.md"), None);
/// ```
pub fn resolve_inside(root: &Path, relative: &str) -> Option<PathBuf> {
    if relative.contains('\0') {
        return None;
    }
    let mut segments: Vec<&std::ffi::OsStr> = Vec::new();

    for component in Path::new(relative).components() {
        match component {
            Component::Normal(segment) => segments.push(segment),
            Component::CurDir => {}
            Component::ParentDir => {
                segments.pop()?;
            }
            Component::Prefix(_) | Component::RootDir => return None,
        }
    }
    if segments.is_empty() {
        return None;
    }
    Some(
        segments
            .iter()
            .fold(root.to_path_buf(), |path, segment| path.join(segment)),
    )
}

/// Chemin de `absolute` relatif à `root`, séparateurs `/` ; `None` s'il n'est pas dedans.
pub fn relative_path(root: &Path, absolute: &Path) -> Option<String> {
    let relative = absolute.strip_prefix(root).ok()?;
    let segments: Vec<String> = relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect();

    Some(segments.join("/"))
}

/// `candidate` est `folder` lui-même ou se trouve dessous : y déplacer `folder` créerait un
/// cycle.
pub fn is_same_or_inside(candidate: &Path, folder: &Path) -> bool {
    candidate.starts_with(folder)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        PathBuf::from("/collections/repo")
    }

    #[test]
    fn resolves_a_path_of_the_collection() {
        assert_eq!(
            resolve_inside(&root(), "docs/a.md"),
            Some(root().join("docs").join("a.md"))
        );
        assert_eq!(
            resolve_inside(&root(), "./docs/../b.md"),
            Some(root().join("b.md"))
        );
    }

    #[test]
    fn refuses_whatever_leaves_the_collection() {
        assert_eq!(resolve_inside(&root(), "../other/a.md"), None);
        assert_eq!(resolve_inside(&root(), "docs/../../x.md"), None);
        assert_eq!(resolve_inside(&root(), "/elsewhere/a.md"), None);
        assert_eq!(resolve_inside(&root(), ""), None);
        assert_eq!(resolve_inside(&root(), "."), None);
        assert_eq!(resolve_inside(&root(), "a\0.md"), None);
    }

    #[cfg(windows)]
    #[test]
    fn refuses_another_drive() {
        assert_eq!(resolve_inside(Path::new("C:\\repo"), "D:\\x.md"), None);
        assert_eq!(resolve_inside(Path::new("C:\\repo"), "D:x.md"), None);
    }

    #[test]
    fn gives_the_relative_path_with_forward_slashes() {
        let absolute = root().join("docs").join("a.md");
        assert_eq!(
            relative_path(&root(), &absolute).as_deref(),
            Some("docs/a.md")
        );
        assert_eq!(relative_path(&root(), Path::new("/elsewhere")), None);
    }

    #[test]
    fn detects_a_move_into_itself() {
        let folder = root().join("docs");
        assert!(is_same_or_inside(&folder, &folder));
        assert!(is_same_or_inside(&folder.join("deep"), &folder));
        assert!(!is_same_or_inside(&root().join("docs2"), &folder));
    }
}
