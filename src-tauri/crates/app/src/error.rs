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
    /// Seules les images d'une collection sont servies à l'aperçu.
    #[error("Not an image")]
    NotAnImage,
    /// Demande d'export malformée.
    #[error("Invalid export request")]
    InvalidExport,
    /// Adresse externe refusée (seuls `http://` et `https://` sont ouverts).
    #[error("Only http:// and https:// URLs are allowed")]
    UnsupportedUrl,
    /// Aucune mise à jour disponible.
    #[error("No update available")]
    NoUpdate,
    /// Profil inconnu.
    #[error("Unknown profile")]
    UnknownProfile,
    /// Mot de passe du profil incorrect (ou mot de passe vide ou trop long).
    #[error("Wrong password")]
    WrongPassword,
    /// Le profil actif est verrouillé : ses données restent fermées jusqu'au mot de passe.
    #[error("The profile is locked")]
    ProfileLocked,
    /// Le profil actif ne reçoit pas un transfert de lui-même.
    #[error("Not possible on the active profile")]
    ActiveProfile,
    /// Icône refusée (format autre que PNG ou WebP, ou trop lourde).
    #[error("Invalid icon")]
    InvalidIcon,
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
    /// Donnée de synchronisation invalide ou fusion refusée.
    #[error(transparent)]
    Sync(#[from] SyncError),
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

/// Erreur de la synchronisation : donnée reçue invalide ou opération refusée par une règle de
/// fusion. Les données reçues étant non fiables, ces erreurs ne sont jamais des paniques.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SyncError {
    /// Update, blob ou state vector au-delà de la taille maximale.
    #[error("sync payload too large: {size} bytes")]
    TooLarge {
        /// Taille reçue, en octets.
        size: usize,
    },
    /// Update illisible ou incohérente.
    #[error("malformed sync update")]
    Malformed,
    /// State vector illisible.
    #[error("malformed state vector")]
    MalformedStateVector,
    /// Update lisible mais refusée par le CRDT.
    #[error("sync update rejected: {0}")]
    Rejected(String),
    /// Fusion d'un profil qui appartient déjà à un espace.
    #[error("profile already synchronized")]
    AlreadySynced,
}

/// Résultat d'une opération de synchronisation pure.
pub type SyncResult<T> = Result<T, SyncError>;

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
