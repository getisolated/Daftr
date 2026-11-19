//! Échange d'un document une fois liés : state vector → diff dans les deux sens,
//! puis vérification de la convergence.

use anyhow::{bail, ensure};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::crdt::NoteDoc;
use crate::message::Message;
use crate::transport::{Role, SecureChannel};

/// Une manche de rattrapage : à la fin, chaque côté a reçu ce qui lui manquait.
/// L'initiateur parle en premier, ce qui évite tout interblocage.
pub async fn exchange<S>(chan: &mut SecureChannel<S>, doc: &NoteDoc) -> anyhow::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    match chan.role {
        Role::Initiator => {
            chan.send(&Message::StateVector(doc.state_vector())).await?;
            receive_diff(chan, doc).await?;
            let remote_sv = receive_state_vector(chan).await?;
            chan.send(&Message::Diff(doc.diff(&remote_sv)?)).await
        }
        Role::Responder => {
            let remote_sv = receive_state_vector(chan).await?;
            chan.send(&Message::Diff(doc.diff(&remote_sv)?)).await?;
            chan.send(&Message::StateVector(doc.state_vector())).await?;
            receive_diff(chan, doc).await
        }
    }
}

/// Compare les contenus des deux côtés ; erreur s'ils diffèrent.
pub async fn verify_convergence<S>(
    chan: &mut SecureChannel<S>,
    doc: &NoteDoc,
) -> anyhow::Result<String>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let local = doc.content();
    chan.send(&Message::Content(local.clone())).await?;
    let Message::Content(remote) = chan.recv().await? else {
        bail!("contenu attendu")
    };
    ensure!(
        remote == local,
        "divergence : local {local:?} ≠ distant {remote:?}"
    );
    Ok(local)
}

async fn receive_state_vector<S>(chan: &mut SecureChannel<S>) -> anyhow::Result<Vec<u8>>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let Message::StateVector(sv) = chan.recv().await? else {
        bail!("state vector attendu")
    };
    Ok(sv)
}

async fn receive_diff<S>(chan: &mut SecureChannel<S>, doc: &NoteDoc) -> anyhow::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let Message::Diff(update) = chan.recv().await? else {
        bail!("diff attendu")
    };
    doc.apply(&update)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::test_support::channel_pair;

    const MERGED: &str = "A modifié\nB 😀\n";

    #[tokio::test]
    async fn two_rounds_converge_over_noise() {
        let (mut b, mut a) = channel_pair().await;
        let (doc_a, doc_b) = (NoteDoc::new(1), NoteDoc::new(2));
        doc_a.set_content("A\n");

        let (ra, rb) = tokio::join!(exchange(&mut a, &doc_a), exchange(&mut b, &doc_b));
        ra.unwrap();
        rb.unwrap();
        doc_a.set_content("A modifié\n");
        doc_b.set_content("A\nB 😀\n");

        let (ra, rb) = tokio::join!(exchange(&mut a, &doc_a), exchange(&mut b, &doc_b));
        ra.unwrap();
        rb.unwrap();
        let (ca, cb) = tokio::join!(
            verify_convergence(&mut a, &doc_a),
            verify_convergence(&mut b, &doc_b)
        );
        assert_eq!(ca.unwrap(), MERGED);
        assert_eq!(cb.unwrap(), MERGED);
    }
}
