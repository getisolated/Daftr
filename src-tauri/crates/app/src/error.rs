//! Erreurs des cas d'usage.

/// Demande refusée par une règle de l'application. Les messages sont stables : le renderer
/// les traduit (`COLLECTION_ERROR_KEYS` de `collections-section.component.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    /// Collection inconnue.
    #[error("Unknown collection")]
    UnknownCollection,
    /// Chemin hors de la collection.
    #[error("Path outside of the collection")]
    OutsideCollection,
    /// Nom de fichier ou de dossier invalide.
    #[error("Invalid name")]
    InvalidName,
    /// Seuls les fichiers markdown s'écrivent.
    #[error("Only markdown files can be written")]
    MarkdownOnly,
    /// Création de fichier hors d'une collection dossier.
    #[error("Files can only be created in a folder collection")]
    CreateInFolderOnly,
    /// Renommage d'une collection fichier.
    #[error("Rename the file from its folder collection")]
    RenameFromFolder,
    /// Déplacement d'un dossier dans lui-même.
    #[error("Cannot move a folder into itself")]
    MoveIntoItself,
    /// Un fichier ou dossier porte déjà ce nom.
    #[error("A file or folder with this name already exists")]
    NameTaken,
    /// Demande d'export malformée.
    #[error("Invalid export request")]
    InvalidExport,
    /// Adresse externe refusée (seuls `http://` et `https://` sont ouverts).
    #[error("Only http:// and https:// URLs are allowed")]
    UnsupportedUrl,
    /// Aucune mise à jour disponible.
    #[error("No update available")]
    NoUpdate,
}

/// Erreur d'un cas d'usage.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// Refus par une règle de l'application.
    #[error(transparent)]
    Refused(#[from] Refusal),
    /// Échec du stockage (base de données).
    #[error("storage failure: {0}")]
    Storage(String),
    /// Échec d'une opération sur le disque, message du système.
    #[error("{0}")]
    FileSystem(String),
}

impl AppError {
    /// Erreur de stockage, avec le message de la cause.
    pub fn storage(cause: impl std::fmt::Display) -> Self {
        Self::Storage(cause.to_string())
    }

    /// Erreur du système de fichiers, avec le message de la cause.
    pub fn file_system(cause: impl std::fmt::Display) -> Self {
        Self::FileSystem(cause.to_string())
    }
}

/// Résultat d'un cas d'usage.
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refusals_keep_the_messages_the_renderer_translates() {
        let error = AppError::from(Refusal::NameTaken);
        assert_eq!(
            error.to_string(),
            "A file or folder with this name already exists"
        );
        assert_eq!(Refusal::UnknownCollection.to_string(), "Unknown collection");
    }
}
