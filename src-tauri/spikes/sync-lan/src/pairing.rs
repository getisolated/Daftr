//! Vérification du code d'appairage : SPAKE2 dans le canal Noise, puis
//! confirmation de clé par un MAC sur le hash du handshake (liaison au canal).
//!
//! L'initiateur (B) confirme en premier ; l'appareil qui écoute (A) ne répond
//! sa propre confirmation qu'après avoir vérifié celle de B.

use anyhow::{Context, bail};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use spake2::{Ed25519Group, Identity as SpakeId, Password, Spake2};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::message::{Message, SessionKind};
use crate::transport::{Role, SecureChannel};

type HmacSha256 = Hmac<Sha256>;

const LABEL_INITIATOR: &[u8] = b"daftr-pair-v1 confirm initiator";
const LABEL_RESPONDER: &[u8] = b"daftr-pair-v1 confirm responder";

/// Issue d'une tentative côté A.
#[derive(Debug, PartialEq, Eq)]
pub enum PairOutcome {
    /// Code correct : les deux appareils sont liés.
    Paired,
    /// Mauvais code : la tentative est refusée.
    WrongCode,
}

/// Côté B : prouve la connaissance du code. Erreur si A refuse.
pub async fn pair_as_initiator<S>(
    chan: &mut SecureChannel<S>,
    local_pub: &[u8],
    code: &str,
) -> anyhow::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    chan.send(&Message::Session(SessionKind::Pair)).await?;
    let ids = Identities::new(Role::Initiator, local_pub, &chan.remote_static);
    let (spake, outbound) = Spake2::<Ed25519Group>::start_a(&Password::new(code), &ids.a, &ids.b);
    chan.send(&Message::Spake(outbound)).await?;

    let Message::Spake(inbound) = chan.recv().await? else {
        bail!("message SPAKE2 attendu")
    };
    let key = spake.finish(&inbound).context("message SPAKE2 invalide")?;

    chan.send(&Message::Confirm(confirmation(
        &key,
        LABEL_INITIATOR,
        &chan.handshake_hash,
    )))
    .await?;
    match chan.recv().await? {
        Message::Confirm(mac) => verify(&key, LABEL_RESPONDER, &chan.handshake_hash, &mac),
        Message::Refused => bail!("code refusé par l'appareil distant"),
        other => bail!("réponse inattendue : {other:?}"),
    }
}

/// Côté A : vérifie le code proposé par B.
pub async fn pair_as_responder<S>(
    chan: &mut SecureChannel<S>,
    local_pub: &[u8],
    code: &str,
) -> anyhow::Result<PairOutcome>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let Message::Session(SessionKind::Pair) = chan.recv().await? else {
        bail!("session Pair attendue")
    };
    let Message::Spake(inbound) = chan.recv().await? else {
        bail!("message SPAKE2 attendu")
    };

    let ids = Identities::new(Role::Responder, local_pub, &chan.remote_static);
    let (spake, outbound) = Spake2::<Ed25519Group>::start_b(&Password::new(code), &ids.a, &ids.b);
    chan.send(&Message::Spake(outbound)).await?;
    let key = spake.finish(&inbound).context("message SPAKE2 invalide")?;

    let Message::Confirm(mac) = chan.recv().await? else {
        bail!("confirmation attendue")
    };
    if verify(&key, LABEL_INITIATOR, &chan.handshake_hash, &mac).is_err() {
        chan.send(&Message::Refused).await?;
        return Ok(PairOutcome::WrongCode);
    }
    chan.send(&Message::Confirm(confirmation(
        &key,
        LABEL_RESPONDER,
        &chan.handshake_hash,
    )))
    .await?;
    Ok(PairOutcome::Paired)
}

/// Identités SPAKE2 : les clés statiques Noise, côté A (initiateur) puis B.
struct Identities {
    a: SpakeId,
    b: SpakeId,
}

impl Identities {
    fn new(role: Role, local: &[u8], remote: &[u8]) -> Self {
        let (initiator, responder) = match role {
            Role::Initiator => (local, remote),
            Role::Responder => (remote, local),
        };
        Self {
            a: SpakeId::new(initiator),
            b: SpakeId::new(responder),
        }
    }
}

fn mac_for(key: &[u8], label: &[u8], handshake_hash: &[u8]) -> HmacSha256 {
    let mut mac =
        HmacSha256::new_from_slice(key).unwrap_or_else(|_| unreachable!("HMAC accepte toute clé"));
    mac.update(label);
    mac.update(handshake_hash);
    mac
}

fn confirmation(key: &[u8], label: &[u8], handshake_hash: &[u8]) -> Vec<u8> {
    mac_for(key, label, handshake_hash)
        .finalize()
        .into_bytes()
        .to_vec()
}

/// Comparaison en temps constant (via `hmac::Mac::verify_slice`).
fn verify(key: &[u8], label: &[u8], handshake_hash: &[u8], received: &[u8]) -> anyhow::Result<()> {
    mac_for(key, label, handshake_hash)
        .verify_slice(received)
        .map_err(|_| anyhow::anyhow!("confirmation de clé invalide"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::test_support::channel_pair;

    async fn attempt(code_a: &str, code_b: &str) -> (anyhow::Result<()>, PairOutcome) {
        let (mut b, mut a) = channel_pair().await;
        let (pub_b, pub_a) = (a.remote_static.clone(), b.remote_static.clone());
        let (res_b, res_a) = tokio::join!(
            pair_as_initiator(&mut b, &pub_b, code_b),
            pair_as_responder(&mut a, &pub_a, code_a)
        );
        (res_b, res_a.unwrap())
    }

    #[tokio::test]
    async fn right_code_pairs_both_sides() {
        let (res_b, outcome_a) = attempt("123456", "123456").await;
        res_b.unwrap();
        assert_eq!(outcome_a, PairOutcome::Paired);
    }

    #[tokio::test]
    async fn wrong_code_is_refused_on_both_sides() {
        let (res_b, outcome_a) = attempt("123456", "123457").await;
        assert!(res_b.is_err());
        assert_eq!(outcome_a, PairOutcome::WrongCode);
    }

    #[test]
    fn confirmation_depends_on_handshake_hash() {
        let key = [1u8; 32];
        let mac = confirmation(&key, LABEL_INITIATOR, b"hash-1");
        assert!(verify(&key, LABEL_INITIATOR, b"hash-1", &mac).is_ok());
        assert!(verify(&key, LABEL_INITIATOR, b"hash-2", &mac).is_err());
        assert!(verify(&key, LABEL_RESPONDER, b"hash-1", &mac).is_err());
    }
}
