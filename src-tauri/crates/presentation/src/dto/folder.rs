//! Dossiers.

use domain::folder::Folder;
use serde::Serialize;
use ts_rs::TS;

/// Un dossier (ligne de la table `folders`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct FolderDto {
    /// Identifiant.
    #[ts(type = "number")]
    pub id: i64,
    /// Nom.
    pub name: String,
    /// Parent, `null` à la racine.
    #[ts(type = "number | null")]
    pub parent_id: Option<i64>,
    /// `1` pour le dossier par défaut, `0` sinon (forme de la colonne SQLite).
    #[ts(type = "0 | 1")]
    pub is_default: u8,
    /// Position sous le parent.
    #[ts(type = "number | null")]
    pub sort_order: Option<i64>,
    /// Couleur `#rrggbb`.
    pub color: Option<String>,
    /// Date de création.
    pub created_at: String,
}

impl From<Folder> for FolderDto {
    fn from(folder: Folder) -> Self {
        Self {
            id: folder.id,
            name: folder.name,
            parent_id: folder.parent_id,
            is_default: u8::from(folder.is_default),
            sort_order: folder.sort_order,
            color: folder.color,
            created_at: folder.created_at,
        }
    }
}
