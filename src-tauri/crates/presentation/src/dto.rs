//! Données échangées avec le renderer, exportées en TypeScript (`cargo test` génère les
//! fichiers dans `frontend/src/app/core/bindings`).
//!
//! Les lignes de la base gardent leurs noms de colonnes (snake_case), comme à l'époque
//! d'Electron ; le reste est en camelCase.

mod collection;
mod folder;
mod note;
mod settings;
mod update;

pub use collection::{
    CollectionDto, CollectionEventDto, CollectionFileRefDto, CollectionGraphDto, CollectionNodeDto,
    FileMatchDto, GraphLinkDto, GraphNodeDto,
};
pub use folder::FolderDto;
pub use note::{ExportedNoteDto, NoteChangesDto, NoteDto};
pub use settings::{OverlayPositionDto, SettingsDto, SettingsUpdateResultDto};
pub use update::{UpdateInfoDto, UpdateProgressDto};
