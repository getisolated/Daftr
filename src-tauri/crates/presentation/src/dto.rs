//! Données échangées avec le renderer, exportées en TypeScript (`cargo test` génère les
//! fichiers dans `frontend/src/app/core/bindings`).
//!
//! Les lignes de la base gardent leurs noms de colonnes (snake_case), comme à l'époque
//! d'Electron ; le reste est en camelCase.

mod collection;
mod collection_sync;
mod folder;
mod mcp;
mod note;
mod profile;
mod settings;
mod sync;
mod update;

pub use collection::{
    CollectionDto, CollectionEventDto, CollectionFileRefDto, CollectionGraphDto, CollectionNodeDto,
    FileMatchDto, GraphLinkDto, GraphNodeDto,
};
pub use collection_sync::{
    CollectionConflictDto, CollectionConflictsDto, CollectionOfferDto, CollectionOffersDto,
    CollectionPeerStateDto, CollectionProgressDto, CollectionShareDeviceDto, CollectionShareDto,
    CollectionShareStatusKind, CollectionStateEventDto, CollectionSyncStateDto, FolderSummaryDto,
    IgnoredFileDto, IgnoredReasonDto,
};
pub use folder::FolderDto;
pub use mcp::{McpErrorKind, McpState, McpStatusDto};
pub use note::{ExportProgressDto, ExportedNoteDto, NoteChangesDto, NoteDto};
pub use profile::{ProfileDto, ProfilesStateDto, TransferResultDto};
pub use settings::{OverlayPositionDto, SettingsDto, SettingsUpdateResultDto};
pub use sync::{
    InitialSyncDoneDto, LinkedDeviceDto, NotesSyncedDto, OfferReceivedDto, PairingStateDto,
    PairingTicketDto, ProfileOfferDto, ProfileSyncDto, SharedProfileDto, SpaceDeviceDto,
    SyncProgressDto, SyncStateDto, SyncStatusDto, SyncStatusKind, SyncedProfileDto, TrustOriginDto,
    TrustedDeviceDto,
};
pub use update::{UpdateInfoDto, UpdateProgressDto};
