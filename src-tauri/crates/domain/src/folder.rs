//! Dossiers de notes (arborescence de la sidebar).

/// Longueur maximale d'un nom de dossier, en caractères.
pub const MAX_FOLDER_NAME_LENGTH: usize = 100;

/// Nom du dossier par défaut, créé s'il manque : les notes créées à la volée y arrivent.
pub const DEFAULT_FOLDER_NAME: &str = "Inbox";

/// Un dossier tel que stocké.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
    /// Identifiant attribué par la base.
    pub id: i64,
    /// Nom affiché.
    pub name: String,
    /// Dossier parent, `None` à la racine.
    pub parent_id: Option<i64>,
    /// Dossier par défaut (`Inbox`) : ni déplaçable ni supprimable.
    pub is_default: bool,
    /// Position choisie par l'utilisateur sous son parent, `None` si jamais positionné.
    pub sort_order: Option<i64>,
    /// Couleur `#rrggbb` de la sidebar, `None` sans couleur.
    pub color: Option<String>,
    /// Date de création (format SQLite).
    pub created_at: String,
}

text_enum! {
    /// Sort des notes d'un dossier supprimé.
    pub enum FolderDeleteMode {
        /// Les notes remontent à la racine.
        MoveToRoot => "moveToRoot",
        /// Les notes sont supprimées avec le dossier.
        DeleteAll => "deleteAll",
    }
}

impl FolderDeleteMode {
    /// Mode demandé par le renderer : tout sauf un `deleteAll` explicite garde les notes,
    /// le choix sûr quand la demande est malformée.
    pub fn from_request(value: Option<&str>) -> Self {
        match value.and_then(Self::parse) {
            Some(Self::DeleteAll) => Self::DeleteAll,
            Some(Self::MoveToRoot) | None => Self::MoveToRoot,
        }
    }
}

/// Le nom sans les espaces autour, `None` s'il est vide ou trop long.
///
/// ```
/// assert_eq!(domain::folder::normalize_folder_name("  Work "), Some("Work".to_owned()));
/// assert_eq!(domain::folder::normalize_folder_name("   "), None);
/// ```
pub fn normalize_folder_name(name: &str) -> Option<String> {
    let trimmed = name.trim();
    let length = trimmed.chars().count();

    (length > 0 && length <= MAX_FOLDER_NAME_LENGTH).then(|| trimmed.to_owned())
}

/// Un dossier peut aller sous `parent_id` sauf s'il y a cycle : `subtree` contient le
/// dossier déplacé et tous ses descendants.
pub fn is_valid_parent(subtree: &[i64], parent_id: Option<i64>) -> bool {
    parent_id.is_none_or(|parent| !subtree.contains(&parent))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_the_name() {
        assert_eq!(normalize_folder_name("  Work  ").as_deref(), Some("Work"));
    }

    #[test]
    fn rejects_empty_and_blank_names() {
        assert_eq!(normalize_folder_name(""), None);
        assert_eq!(normalize_folder_name("   "), None);
    }

    #[test]
    fn accepts_exactly_the_limit_and_rejects_beyond() {
        let at_limit = "é".repeat(MAX_FOLDER_NAME_LENGTH);
        assert!(normalize_folder_name(&at_limit).is_some());
        assert_eq!(
            normalize_folder_name(&"a".repeat(MAX_FOLDER_NAME_LENGTH + 1)),
            None
        );
    }

    #[test]
    fn deletes_notes_only_on_an_explicit_request() {
        assert_eq!(
            FolderDeleteMode::from_request(Some("deleteAll")),
            FolderDeleteMode::DeleteAll
        );
        assert_eq!(
            FolderDeleteMode::from_request(Some("moveToRoot")),
            FolderDeleteMode::MoveToRoot
        );
        assert_eq!(
            FolderDeleteMode::from_request(Some("everything")),
            FolderDeleteMode::MoveToRoot
        );
        assert_eq!(
            FolderDeleteMode::from_request(None),
            FolderDeleteMode::MoveToRoot
        );
    }

    #[test]
    fn refuses_to_move_a_folder_into_its_own_subtree() {
        let subtree = [1, 4, 7];
        assert!(is_valid_parent(&subtree, None));
        assert!(is_valid_parent(&subtree, Some(2)));
        assert!(!is_valid_parent(&subtree, Some(1)));
        assert!(!is_valid_parent(&subtree, Some(7)));
    }
}
