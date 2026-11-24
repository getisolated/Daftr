//! Évaluation d'un manifeste distant : plan du fichier par fichier, et comptabilité immédiate
//! (versions identiques retenues, absences oubliées) ; le reste est une file de transferts.

use app::AppResult;
use app::collection_sync::{FileAction, FileEntry, Ignored, PlannedFile, plan};

use super::apply::{adopt, forget};
use super::{LocalSummary, Ports, ShareCtx, now_nanos, snapshot};

/// Ce que cet appareil doit faire pour rejoindre l'état de l'autre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Evaluation {
    /// Résumé du dossier local, fichiers écartés compris.
    pub summary: LocalSummary,
    /// Les fichiers à recevoir, fusionner ou supprimer, dans l'ordre des chemins.
    pub queue: Vec<PlannedFile>,
}

/// Compare le dossier local au manifeste distant. `None` si le dossier n'existe plus.
pub(crate) fn evaluate(
    ports: &Ports<'_>,
    ctx: &ShareCtx<'_>,
    remote: &[FileEntry],
) -> AppResult<Option<Evaluation>> {
    let Some(snapshot) = snapshot(ports, ctx, now_nanos())? else {
        return Ok(None);
    };
    let mut summary = snapshot.summary;
    let mut queue = Vec::new();

    for item in plan(&snapshot.manifest, remote) {
        match item.action {
            FileAction::Adopt => {
                if let Some(hash) = item.remote {
                    adopt(ports, ctx, &item.path, hash, item.size)?;
                }
            }
            FileAction::Forget => forget(ports, ctx, &item.path)?,
            FileAction::Skip(reason) => summary.ignored.push(Ignored {
                path: item.path.as_str().to_owned(),
                reason,
            }),
            FileAction::Pull | FileAction::DeleteLocal | FileAction::Reconcile => queue.push(item),
        }
    }
    Ok(Some(Evaluation { summary, queue }))
}
