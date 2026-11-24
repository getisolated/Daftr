//! Les collections partagées préparées pour le renderer : état d'un partage, copies de
//! conflit, résumé d'un dossier.

use std::collections::HashMap;
use std::path::Path;

use adapter_fs::ShareDisk;
use adapter_sync::CollectionStatus;
use app::collection_sync::{Ignored, IgnoredReason, parse_conflict};
use app::ports::collection_sync::{CollectionDisk, SharePeerState, ShareRecord};
use domain::profile::ProfileId;
use domain::sync::DeviceKey;
use presentation::dto::{
    CollectionConflictDto, CollectionPeerStateDto, CollectionShareDeviceDto, CollectionShareDto,
    CollectionShareStatusKind, FolderSummaryDto, IgnoredFileDto, IgnoredReasonDto,
};

/// Tout ce qu'il faut pour décrire un partage.
pub(super) struct ShareView<'a> {
    pub profile: &'a ProfileId,
    pub record: &'a ShareRecord,
    pub status: CollectionStatus,
    /// Nom de chaque appareil de confiance.
    pub names: &'a HashMap<DeviceKey, String>,
    /// Nom de cet appareil.
    pub my_name: &'a str,
}

pub(super) fn share_dto(view: ShareView<'_>) -> CollectionShareDto {
    let ShareView {
        profile,
        record,
        status,
        names,
        my_name,
    } = view;
    let syncing: Vec<_> = status.peers.iter().filter(|peer| peer.syncing).collect();
    let devices = record
        .peers
        .iter()
        .map(|peer| {
            let live = status.peers.iter().find(|live| live.device == peer.device);
            CollectionShareDeviceDto {
                key: peer.device.to_string(),
                name: names
                    .get(&peer.device)
                    .cloned()
                    .unwrap_or_else(|| short_key(&peer.device)),
                state: match peer.state {
                    SharePeerState::Offered => CollectionPeerStateDto::Offered,
                    SharePeerState::Active => CollectionPeerStateDto::Active,
                },
                connected: live.is_some_and(|live| live.connected),
                syncing: live.is_some_and(|live| live.syncing),
                last_sync_at: peer.last_sync_at.clone(),
            }
        })
        .collect();

    CollectionShareDto {
        profile_id: profile.as_str().to_owned(),
        sync_id: record.sync_id.to_string(),
        root: record.root.clone(),
        name: record.name.clone(),
        status: status_kind(record, &status),
        files: status.files as u64,
        done: syncing.iter().map(|peer| peer.done).sum(),
        total: syncing.iter().map(|peer| peer.total).sum(),
        devices,
        conflicts: conflicts_of(&status.conflicts, my_name),
        ignored: status.ignored.iter().map(ignored_dto).collect(),
        truncated: status.truncated,
    }
}

/// L'état de la pastille : le dossier manquant l'emporte, puis l'échange en cours, les
/// conflits, l'absence des appareils, la proposition sans réponse, enfin « à jour ».
fn status_kind(record: &ShareRecord, status: &CollectionStatus) -> CollectionShareStatusKind {
    let active: Vec<DeviceKey> = record
        .peers
        .iter()
        .filter(|peer| peer.state == SharePeerState::Active)
        .map(|peer| peer.device)
        .collect();
    let connected = |key: &DeviceKey| {
        status
            .peers
            .iter()
            .any(|peer| peer.device == *key && peer.connected)
    };

    if status.root_missing {
        CollectionShareStatusKind::MissingFolder
    } else if status.peers.iter().any(|peer| peer.syncing) {
        CollectionShareStatusKind::Syncing
    } else if !status.conflicts.is_empty() {
        CollectionShareStatusKind::Conflicts
    } else if active.is_empty() {
        CollectionShareStatusKind::Offered
    } else if !active.iter().any(connected) {
        CollectionShareStatusKind::Waiting
    } else {
        CollectionShareStatusKind::UpToDate
    }
}

/// Les copies de conflit, avec l'appareil dont elles gardent la version.
pub(super) fn conflicts_of(paths: &[String], my_name: &str) -> Vec<CollectionConflictDto> {
    paths
        .iter()
        .filter_map(|path| {
            let name = parse_conflict(path)?;
            Some(CollectionConflictDto {
                path: path.clone(),
                original: name.original.clone(),
                mine: name.is_from(my_name),
                device_name: name.label,
            })
        })
        .collect()
}

fn ignored_dto(ignored: &Ignored) -> IgnoredFileDto {
    let reason = match ignored.reason {
        IgnoredReason::TooLarge => IgnoredReasonDto::TooLarge,
        IgnoredReason::CaseCollision => IgnoredReasonDto::CaseCollision,
        IgnoredReason::InvalidPath => IgnoredReasonDto::InvalidPath,
        IgnoredReason::Unreadable => IgnoredReasonDto::Unreadable,
    };
    IgnoredFileDto {
        path: ignored.path.clone(),
        reason,
    }
}

/// Début de l'identifiant d'un appareil dont le nom est inconnu.
pub(super) fn short_key(key: &DeviceKey) -> String {
    key.to_string().chars().take(8).collect()
}

/// Ce que contient un dossier de synchronisable : fichiers `.md`, volume, git, écartés.
pub(super) fn folder_summary(disk: &ShareDisk, path: &Path) -> FolderSummaryDto {
    let exists = disk.is_directory(path);
    if !exists {
        return FolderSummaryDto {
            exists,
            empty: true,
            md_files: 0,
            bytes: 0,
            has_git: false,
            ignored: Vec::new(),
            truncated: false,
        };
    }
    let scan = disk.scan(path).unwrap_or_default();
    FolderSummaryDto {
        exists,
        empty: disk.is_empty_directory(path),
        md_files: scan.files.len() as u64,
        bytes: scan.files.iter().map(|file| file.stat.size).sum(),
        has_git: path.join(".git").exists(),
        ignored: scan.ignored.iter().map(ignored_dto).collect(),
        truncated: scan.truncated,
    }
}
