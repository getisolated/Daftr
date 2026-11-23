//! Vérification du code d'appairage : SPAKE2 dans le canal Noise, liée à la connexion.
//!
//! Construction (à reproduire à l'identique par tout autre client) :
//! 1. SPAKE2 `spake2` 0.4, groupe Ed25519 (variante de `python-spake2`), mot de passe = les 6
//!    chiffres ASCII du code, identité A = clé statique Noise de l'appareil qui compose,
//!    identité B = celle de l'appareil qui écoute (32 octets bruts chacune). Celui qui compose
//!    envoie `Spake(start_a)`, l'autre répond `Spake(start_b)`.
//! 2. Clé partagée K (32 octets) ; confirmation = HMAC-SHA256(K, étiquette ‖ h), où h est le
//!    hash du handshake Noise (32 octets) et l'étiquette `daftr-pair-v1 confirm initiator` ou
//!    `… responder`. L'appareil qui compose confirme en premier ; l'autre ne renvoie sa
//!    confirmation qu'après avoir vérifié la première, sinon `Refused(WrongCode)`.
//!
//! `spake2` 0.4 n'a pas de paramètre de contexte : la liaison au canal passe par les identités
//! (clés authentifiées par Noise) et par h dans la confirmation. Chaque connexion n'offre qu'un
//! essai, et la transcription ne permet aucune attaque hors ligne sur le code.

use domain::sync::{DeviceKey, PairingCode};
use hmac::{Hmac, Mac};
use sha2::Sha256;
use spake2::{Ed25519Group, Identity, Password, Spake2};
use tokio::io::{AsyncRead, AsyncWrite};

use crate::error::{Error, Result};
use crate::noise::Channel;
use crate::wire::{Message, PairRefusal};

type HmacSha256 = Hmac<Sha256>;

const LABEL_INITIATOR: &[u8] = b"daftr-pair-v1 confirm initiator";
const LABEL_RESPONDER: &[u8] = b"daftr-pair-v1 confirm responder";

/// Issue d'une vérification côté appareil qui écoute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// Code correct : les deux appareils peuvent se lier.
    Paired,
    /// Mauvais code : tentative refusée (et signalée à l'autre appareil).
    WrongCode,
}

/// Côté qui compose : prouve la connaissance du code et vérifie la réponse.
pub(crate) async fn prove<S>(
    channel: &mut Channel<S>,
    local: &DeviceKey,
    code: &PairingCode,
) -> Result<()>
where
    S: AsyncRead + AsyncWrite,
{
    let (spake, outbound) = Spake2::<Ed25519Group>::start_a(
        &Password::new(code.as_str()),
        &Identity::new(local.as_bytes()),
        &Identity::new(channel.remote.as_bytes()),
    );
    channel.send(&Message::Spake(outbound)).await?;
    let inbound = match channel.recv().await? {
        Message::Spake(inbound) => inbound,
        Message::Refused(refusal) => return Err(refused(refusal)),
        _ => return Err(Error::Unexpected),
    };
    let key = spake.finish(&inbound).map_err(|_| Error::Malformed)?;

    let mine = confirmation(&key, LABEL_INITIATOR, &channel.handshake_hash);
    channel.send(&Message::Confirm(mine)).await?;
    match channel.recv().await? {
        Message::Confirm(theirs) => verify(&key, LABEL_RESPONDER, &channel.handshake_hash, &theirs),
        Message::Refused(refusal) => Err(refused(refusal)),
        _ => Err(Error::Unexpected),
    }
}

/// Côté qui écoute : vérifie le code proposé par l'appareil qui compose. Sur un mauvais code,
/// l'appelant compte l'échec puis répond `Refused(WrongCode)` (voir [`refuse`]).
pub(crate) async fn verify_code<S>(
    channel: &mut Channel<S>,
    local: &DeviceKey,
    code: &PairingCode,
) -> Result<Verdict>
where
    S: AsyncRead + AsyncWrite,
{
    let Message::Spake(inbound) = channel.recv().await? else {
        return Err(Error::Unexpected);
    };
    let (spake, outbound) = Spake2::<Ed25519Group>::start_b(
        &Password::new(code.as_str()),
        &Identity::new(channel.remote.as_bytes()),
        &Identity::new(local.as_bytes()),
    );
    channel.send(&Message::Spake(outbound)).await?;
    let key = spake.finish(&inbound).map_err(|_| Error::Malformed)?;

    let Message::Confirm(theirs) = channel.recv().await? else {
        return Err(Error::Unexpected);
    };
    if verify(&key, LABEL_INITIATOR, &channel.handshake_hash, &theirs).is_err() {
        return Ok(Verdict::WrongCode);
    }
    let mine = confirmation(&key, LABEL_RESPONDER, &channel.handshake_hash);
    channel.send(&Message::Confirm(mine)).await?;
    Ok(Verdict::Paired)
}

/// Refuse la tentative en cours.
pub(crate) async fn refuse<S>(channel: &mut Channel<S>, refusal: PairRefusal) -> Result<()>
where
    S: AsyncRead + AsyncWrite,
{
    channel.send(&Message::Refused(refusal)).await
}

fn refused(refusal: PairRefusal) -> Error {
    match refusal {
        PairRefusal::NotPairing => Error::NotPairing,
        PairRefusal::WrongCode => Error::WrongCode,
    }
}

fn mac(key: &[u8], label: &[u8], handshake_hash: &[u8]) -> Result<HmacSha256> {
    let mut mac = HmacSha256::new_from_slice(key).map_err(|_| Error::Malformed)?;
    mac.update(label);
    mac.update(handshake_hash);
    Ok(mac)
}

fn confirmation(key: &[u8], label: &[u8], handshake_hash: &[u8]) -> Vec<u8> {
    // HMAC accepte une clé de toute longueur : l'échec est impossible, la confirmation vide
    // serait de toute façon refusée par l'autre appareil
    mac(key, label, handshake_hash)
        .map(|mac| mac.finalize().into_bytes().to_vec())
        .unwrap_or_default()
}

/// Comparaison en temps constant.
fn verify(key: &[u8], label: &[u8], handshake_hash: &[u8], received: &[u8]) -> Result<()> {
    mac(key, label, handshake_hash)?
        .verify_slice(received)
        .map_err(|_| Error::WrongCode)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::DeviceIdentity;
    use crate::noise::test_support::channel_pair;

    async fn attempt(code_a: &str, code_b: &str) -> (Result<()>, Result<Verdict>) {
        let (b, a) = (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        );
        let (mut cb, mut ca) = channel_pair(&b, &a).await;
        let (code_a, code_b) = (
            PairingCode::parse(code_a).unwrap(),
            PairingCode::parse(code_b).unwrap(),
        );
        let (key_a, key_b) = (a.key(), b.key());
        let verify = async {
            let verdict = verify_code(&mut ca, &key_a, &code_a).await?;
            if verdict == Verdict::WrongCode {
                refuse(&mut ca, PairRefusal::WrongCode).await?;
            }
            Ok(verdict)
        };
        tokio::join!(prove(&mut cb, &key_b, &code_b), verify)
    }

    #[tokio::test]
    async fn the_right_code_pairs_both_sides() {
        let (proved, verdict) = attempt("123456", "123456").await;
        proved.unwrap();
        assert_eq!(verdict.unwrap(), Verdict::Paired);
    }

    #[tokio::test]
    async fn a_wrong_code_is_refused_on_both_sides() {
        let (proved, verdict) = attempt("123456", "123457").await;
        assert!(matches!(proved, Err(Error::WrongCode)));
        assert_eq!(verdict.unwrap(), Verdict::WrongCode);
    }

    #[test]
    fn the_confirmation_is_bound_to_the_handshake_and_the_role() {
        let key = [1_u8; 32];
        let sent = confirmation(&key, LABEL_INITIATOR, b"hash-1");
        assert!(verify(&key, LABEL_INITIATOR, b"hash-1", &sent).is_ok());
        assert!(verify(&key, LABEL_INITIATOR, b"hash-2", &sent).is_err());
        assert!(verify(&key, LABEL_RESPONDER, b"hash-1", &sent).is_err());
        assert!(verify(&[2; 32], LABEL_INITIATOR, b"hash-1", &sent).is_err());
    }
}
