//! Parcours d'une collection partagée : manifeste local et résumé affiché à l'utilisateur.

use std::collections::{HashMap, HashSet};

use app::AppResult;
use app::collection_sync::{
    FileHash, Ignored, IgnoredReason, LocalFile, Manifest, SyncPath, build_manifest, parse_conflict,
};
use app::ports::collection_sync::{IndexRow, LocalScan, ScannedFile};

use super::{Ports, ShareCtx};

/// Un fichier modifié depuis moins que ça n'est pas relu depuis l'index : certains systèmes de
/// fichiers datent les écritures à la seconde, et deux écritures de même taille dans la même
/// seconde donneraient la même date.
const FRESH_NANOS: i64 = 2_000_000_000;

/// Ce que l'interface montre d'une collection partagée.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LocalSummary {
    /// Fichiers synchronisés.
    pub files: usize,
    /// Fichiers écartés, avec la raison.
    pub ignored: Vec<Ignored>,
    /// Copies de conflit présentes (chemins relatifs).
    pub conflicts: Vec<String>,
    /// Le parcours s'est arrêté à une limite.
    pub truncated: bool,
    /// Le dossier n'existe plus.
    pub root_missing: bool,
}

/// L'état local d'un partage à un instant donné.
pub(crate) struct Snapshot {
    /// Ce qu'on annonce à l'appareil distant.
    pub manifest: Manifest,
    /// Ce qu'on affiche.
    pub summary: LocalSummary,
}

/// Parcourt la collection et hache les fichiers qui ont changé depuis l'index. `None` si le
/// dossier n'existe plus.
pub(crate) fn snapshot(
    ports: &Ports<'_>,
    ctx: &ShareCtx<'_>,
    now_nanos: i64,
) -> AppResult<Option<Snapshot>> {
    if !ports.disk.is_directory(ctx.root) {
        return Ok(None);
    }
    let scan = ports.disk.scan(ctx.root)?;
    let index = ports.store.file_index(&ctx.id)?;
    let (files, rows, unreadable) = hash_files(ports, ctx, &scan, &index, now_nanos);
    if index_changed(&index, &rows) {
        ports.store.replace_file_index(&ctx.id, &rows)?;
    }
    let excludes = ports.disk.excludes();
    let bases = ports.store.bases(&ctx.id, &ctx.remote)?;
    let bases: HashMap<SyncPath, FileHash> = bases
        .into_iter()
        .filter_map(|(path, hash)| Some((SyncPath::parse(&path, &excludes).ok()?, hash)))
        .collect();

    let mut manifest = build_manifest(files, &bases);
    manifest.ignored.extend(scan.ignored.iter().cloned());
    manifest.ignored.extend(unreadable);
    let summary = summarize(&manifest, scan.truncated);
    Ok(Some(Snapshot { manifest, summary }))
}

type Hashed = (Vec<LocalFile>, Vec<IndexRow>, Vec<Ignored>);

fn hash_files(
    ports: &Ports<'_>,
    ctx: &ShareCtx<'_>,
    scan: &LocalScan,
    index: &HashMap<String, IndexRow>,
    now_nanos: i64,
) -> Hashed {
    let (mut files, mut rows, mut unreadable) = (Vec::new(), Vec::new(), Vec::new());

    for scanned in &scan.files {
        match hash_of(ports, ctx, scanned, index, now_nanos) {
            Some(hash) => {
                let stat = scanned.stat;
                rows.push(IndexRow {
                    path: scanned.path.as_str().to_owned(),
                    hash,
                    size: stat.size,
                    mtime: stat.mtime,
                });
                files.push(LocalFile {
                    path: scanned.path.clone(),
                    hash,
                    size: stat.size,
                });
            }
            None => unreadable.push(Ignored {
                path: scanned.path.as_str().to_owned(),
                reason: IgnoredReason::Unreadable,
            }),
        }
    }
    (files, rows, unreadable)
}

/// L'empreinte du fichier : celle de l'index s'il n'a pas bougé, sinon relue.
fn hash_of(
    ports: &Ports<'_>,
    ctx: &ShareCtx<'_>,
    scanned: &ScannedFile,
    index: &HashMap<String, IndexRow>,
    now_nanos: i64,
) -> Option<FileHash> {
    let stat = scanned.stat;
    let known = index.get(scanned.path.as_str()).filter(|row| {
        row.size == stat.size && row.mtime == stat.mtime && now_nanos - stat.mtime > FRESH_NANOS
    });
    if let Some(row) = known {
        return Some(row.hash);
    }
    let bytes = ports.disk.read(ctx.root, &scanned.path).ok()?;
    Some(FileHash::of(&bytes))
}

fn index_changed(index: &HashMap<String, IndexRow>, rows: &[IndexRow]) -> bool {
    index.len() != rows.len() || rows.iter().any(|row| index.get(&row.path) != Some(row))
}

fn summarize(manifest: &Manifest, truncated: bool) -> LocalSummary {
    let present: HashSet<&str> = manifest
        .entries
        .iter()
        .filter(|entry| entry.hash.is_some())
        .map(|entry| entry.path.as_str())
        .collect();
    let conflicts = present
        .iter()
        .filter(|path| {
            parse_conflict(path).is_some_and(|name| present.contains(name.original.as_str()))
        })
        .map(|path| (*path).to_owned())
        .collect::<Vec<_>>();

    LocalSummary {
        files: present.len(),
        ignored: manifest.ignored.clone(),
        conflicts: sorted(conflicts),
        truncated,
        root_missing: false,
    }
}

fn sorted(mut items: Vec<String>) -> Vec<String> {
    items.sort();
    items
}
