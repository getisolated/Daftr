//! Exécution d'une décision sur un fichier : écriture, suppression, conflit, mise à jour de
//! la dernière version échangée.
//!
//! Aucune écriture n'a lieu si le fichier local n'a plus l'empreinte que le plan a vue : une
//! modification de l'utilisateur pendant le transfert n'est jamais écrasée (le prochain
//! manifeste relance la décision).

use app::AppResult;
use app::collection_sync::{FileHash, PlannedFile, Resolution, SyncPath, conflict_name, resolve};
use app::ports::collection_sync::{BaseRow, IndexRow};

use super::{Ports, ShareCtx, base_text, local_hash};

/// Ce qu'une décision a fait, pour la session.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Outcome {
    /// Accusé à envoyer à l'appareil : la version que nous avons maintenant (`None` : plus de
    /// fichier).
    pub ack: Option<Option<FileHash>>,
    /// Versions remplacées, à garder pour les demandes de l'autre appareil.
    pub replaced: Vec<(String, FileHash, Vec<u8>)>,
    /// Copie de conflit créée.
    pub conflict_copy: Option<String>,
    /// Les fichiers ont changé ici.
    pub changed: bool,
    /// Rien n'a été fait : contenu invalide, ou fichier modifié depuis le plan.
    pub skipped: bool,
}

impl Outcome {
    fn skipped() -> Self {
        Self {
            skipped: true,
            ..Self::default()
        }
    }
}

/// Écrit la version distante d'un fichier (`Pull`).
pub(crate) fn pull(
    ports: &Ports<'_>,
    ctx: &ShareCtx<'_>,
    item: &PlannedFile,
    bytes: &[u8],
) -> AppResult<Outcome> {
    let hash = FileHash::of(bytes);
    if item.remote != Some(hash) || !unchanged(ports, ctx, item)? {
        return Ok(Outcome::skipped());
    }
    let mut outcome = Outcome::default();
    replace(ports, ctx, &item.path, &mut outcome)?;
    write(ports, ctx, &item.path, bytes)?;
    ports.store.set_base(
        &ctx.id,
        &ctx.remote,
        item.path.as_str(),
        &BaseRow {
            hash,
            text: base_text(bytes),
        },
    )?;
    outcome.ack = Some(Some(hash));
    outcome.changed = true;
    Ok(outcome)
}

/// Envoie le fichier local à la corbeille (`DeleteLocal`).
pub(crate) fn delete_local(
    ports: &Ports<'_>,
    ctx: &ShareCtx<'_>,
    item: &PlannedFile,
) -> AppResult<Outcome> {
    if !unchanged(ports, ctx, item)? {
        return Ok(Outcome::skipped());
    }
    let path = item.path.as_str();
    let mut outcome = Outcome::default();
    replace(ports, ctx, &item.path, &mut outcome)?;
    ports.disk.trash(ctx.root, &item.path)?;
    ports.store.drop_index_row(&ctx.id, path)?;
    ports.store.clear_base(&ctx.id, &ctx.remote, path)?;
    outcome.ack = Some(None);
    outcome.changed = true;
    Ok(outcome)
}

/// Résout un conflit (`Reconcile`) : fusion identique des deux côtés, sinon l'arbitre garde sa
/// version et le suiveur met la sienne dans une copie avant de prendre celle de l'arbitre.
pub(crate) fn reconcile(
    ports: &Ports<'_>,
    ctx: &ShareCtx<'_>,
    item: &PlannedFile,
    remote: &[u8],
) -> AppResult<Outcome> {
    let remote_hash = FileHash::of(remote);
    if item.remote != Some(remote_hash) || !unchanged(ports, ctx, item)? {
        return Ok(Outcome::skipped());
    }
    let local = ports.disk.read(ctx.root, &item.path)?;
    let base = common_base_text(ports, ctx, item)?;

    match resolve(ctx.role, base.as_deref(), &local, remote) {
        Resolution::Merged(merged) => take(ports, ctx, item, &merged),
        Resolution::KeepBoth if ctx.role == app::collection_sync::Role::Follower => {
            keep_both(ports, ctx, item, &local, remote)
        }
        Resolution::KeepBoth => Ok(Outcome::default()),
    }
}

/// La version fusionnée remplace le fichier ; les deux côtés l'écrivent.
fn take(
    ports: &Ports<'_>,
    ctx: &ShareCtx<'_>,
    item: &PlannedFile,
    bytes: &[u8],
) -> AppResult<Outcome> {
    let hash = FileHash::of(bytes);
    let mut outcome = Outcome::default();
    if Some(hash) != item.local {
        replace(ports, ctx, &item.path, &mut outcome)?;
        write(ports, ctx, &item.path, bytes)?;
        outcome.changed = true;
    }
    let base = BaseRow {
        hash,
        text: base_text(bytes),
    };
    ports
        .store
        .set_base(&ctx.id, &ctx.remote, item.path.as_str(), &base)?;
    outcome.ack = Some(Some(hash));
    Ok(outcome)
}

/// La copie (version locale) d'abord, puis la version de l'arbitre à la place d'origine : un
/// arrêt entre les deux ne perd rien.
fn keep_both(
    ports: &Ports<'_>,
    ctx: &ShareCtx<'_>,
    item: &PlannedFile,
    local: &[u8],
    remote: &[u8],
) -> AppResult<Outcome> {
    let copy = conflict_name(&item.path, ctx.my_name, |candidate| {
        SyncPath::parse(candidate, &ports.disk.excludes())
            .map_or(true, |path| ports.disk.exists(ctx.root, &path))
    });
    if copy == item.path || ports.disk.exists(ctx.root, &copy) {
        return Ok(Outcome::skipped());
    }
    write(ports, ctx, &copy, local)?;
    let mut outcome = Outcome::default();
    replace(ports, ctx, &item.path, &mut outcome)?;
    write(ports, ctx, &item.path, remote)?;
    let hash = FileHash::of(remote);
    let base = BaseRow {
        hash,
        text: base_text(remote),
    };
    ports
        .store
        .set_base(&ctx.id, &ctx.remote, item.path.as_str(), &base)?;
    outcome.ack = Some(Some(hash));
    outcome.conflict_copy = Some(copy.as_str().to_owned());
    outcome.changed = true;
    Ok(outcome)
}

/// Même contenu des deux côtés : retenu comme dernière version échangée.
pub(crate) fn adopt(
    ports: &Ports<'_>,
    ctx: &ShareCtx<'_>,
    path: &SyncPath,
    hash: FileHash,
    size: u64,
) -> AppResult<()> {
    let small = size <= u64::try_from(app::collection_sync::MAX_BASE_TEXT_BYTES).unwrap_or(0);
    let text = if small && local_hash(ports, ctx.root, path)? == Some(hash) {
        base_text(&ports.disk.read(ctx.root, path)?)
    } else {
        None
    };
    ports
        .store
        .set_base(&ctx.id, &ctx.remote, path.as_str(), &BaseRow { hash, text })
}

/// Absent des deux côtés : la dernière version échangée est oubliée.
pub(crate) fn forget(ports: &Ports<'_>, ctx: &ShareCtx<'_>, path: &SyncPath) -> AppResult<()> {
    ports.store.clear_base(&ctx.id, &ctx.remote, path.as_str())
}

/// L'appareil distant a maintenant la version `hash` du fichier (ou plus de fichier).
pub(crate) fn acked(
    ports: &Ports<'_>,
    ctx: &ShareCtx<'_>,
    path: &SyncPath,
    hash: Option<FileHash>,
) -> AppResult<()> {
    let local = local_hash(ports, ctx.root, path)?;
    let Some(hash) = hash else {
        if local.is_none() {
            ports
                .store
                .clear_base(&ctx.id, &ctx.remote, path.as_str())?;
        }
        return Ok(());
    };
    let text = match local {
        Some(current) if current == hash => base_text(&ports.disk.read(ctx.root, path)?),
        _ => None,
    };
    ports
        .store
        .set_base(&ctx.id, &ctx.remote, path.as_str(), &BaseRow { hash, text })
}

/// Le fichier local a toujours l'empreinte vue par le plan.
fn unchanged(ports: &Ports<'_>, ctx: &ShareCtx<'_>, item: &PlannedFile) -> AppResult<bool> {
    Ok(local_hash(ports, ctx.root, &item.path)? == item.local)
}

/// Le texte de la base commune, si on le garde et qu'il est bien celui que le plan désigne.
fn common_base_text(
    ports: &Ports<'_>,
    ctx: &ShareCtx<'_>,
    item: &PlannedFile,
) -> AppResult<Option<Vec<u8>>> {
    let Some(wanted) = item.base else {
        return Ok(None);
    };
    let stored = ports.store.base(&ctx.id, &ctx.remote, item.path.as_str())?;
    Ok(stored
        .filter(|row| row.hash == wanted)
        .and_then(|row| row.text)
        .filter(|text| FileHash::of(text) == wanted))
}

/// Garde la version qui va être remplacée (pour l'autre appareil, qui peut la demander).
fn replace(
    ports: &Ports<'_>,
    ctx: &ShareCtx<'_>,
    path: &SyncPath,
    outcome: &mut Outcome,
) -> AppResult<()> {
    if ports.disk.exists(ctx.root, path) {
        let old = ports.disk.read(ctx.root, path)?;
        outcome
            .replaced
            .push((path.as_str().to_owned(), FileHash::of(&old), old));
    }
    Ok(())
}

/// Écriture atomique, puis index à jour : le prochain parcours n'y verra pas une modification.
fn write(ports: &Ports<'_>, ctx: &ShareCtx<'_>, path: &SyncPath, bytes: &[u8]) -> AppResult<()> {
    let stat = ports.disk.write(ctx.root, path, bytes)?;
    ports.store.put_index_row(
        &ctx.id,
        &IndexRow {
            path: path.as_str().to_owned(),
            hash: FileHash::of(bytes),
            size: stat.size,
            mtime: stat.mtime,
        },
    )
}
