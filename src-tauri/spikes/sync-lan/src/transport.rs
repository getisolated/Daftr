//! Transport : trames préfixées par leur longueur, handshake Noise XX,
//! messages `postcard` découpés en trames Noise (65 535 octets au plus chacune).

use anyhow::{Context, bail, ensure};
use serde::{Serialize, de::DeserializeOwned};
use snow::{HandshakeState, TransportState};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::identity::{Identity, NOISE_PARAMS};

/// Taille maximale d'un message applicatif (§7) : au-delà, on rejette.
pub const MAX_MESSAGE_LEN: usize = 16 * 1024 * 1024;

const NOISE_MAX_FRAME: usize = 65_535;
const NOISE_TAG_LEN: usize = 16;
const MAX_CHUNK: usize = NOISE_MAX_FRAME - NOISE_TAG_LEN;

/// Rôle Noise : l'initiateur est celui qui compose la connexion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// Côté qui se connecte (B).
    Initiator,
    /// Côté qui écoute (A).
    Responder,
}

/// Canal chiffré établi après le handshake Noise XX.
pub struct SecureChannel<S> {
    stream: S,
    noise: TransportState,
    /// Hash du handshake Noise : lie l'appairage à cette connexion précise.
    pub handshake_hash: Vec<u8>,
    /// Clé statique distante, authentifiée par le handshake.
    pub remote_static: Vec<u8>,
    /// Rôle local.
    pub role: Role,
}

/// Effectue le handshake Noise XX sur `stream`.
pub async fn handshake<S>(
    stream: S,
    identity: &Identity,
    role: Role,
) -> anyhow::Result<SecureChannel<S>>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let builder = snow::Builder::new(NOISE_PARAMS.parse()?).local_private_key(&identity.private)?;
    let state = match role {
        Role::Initiator => builder.build_initiator()?,
        Role::Responder => builder.build_responder()?,
    };
    let mut stream = stream;
    let state = drive_handshake(&mut stream, state).await?;
    into_channel(stream, state, role)
}

async fn drive_handshake<S>(
    stream: &mut S,
    mut state: HandshakeState,
) -> anyhow::Result<HandshakeState>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut buf = vec![0u8; NOISE_MAX_FRAME];
    while !state.is_handshake_finished() {
        if state.is_my_turn() {
            let len = state.write_message(&[], &mut buf)?;
            write_frame(stream, &buf[..len]).await?;
        } else {
            let frame = read_frame(stream).await?;
            state.read_message(&frame, &mut buf)?;
        }
    }
    Ok(state)
}

/// Le hash du handshake n'est plus accessible après `into_transport_mode` :
/// il faut le copier avant.
fn into_channel<S>(
    stream: S,
    state: HandshakeState,
    role: Role,
) -> anyhow::Result<SecureChannel<S>> {
    let handshake_hash = state.get_handshake_hash().to_vec();
    let remote_static = state
        .get_remote_static()
        .context("clé statique distante absente")?
        .to_vec();
    let noise = state.into_transport_mode()?;
    Ok(SecureChannel {
        stream,
        noise,
        handshake_hash,
        remote_static,
        role,
    })
}

impl<S: AsyncRead + AsyncWrite + Unpin> SecureChannel<S> {
    /// Sérialise `message` en `postcard` et l'envoie, découpé en trames Noise.
    pub async fn send<T: Serialize>(&mut self, message: &T) -> anyhow::Result<()> {
        let body = postcard::to_allocvec(message)?;
        ensure!(
            body.len() <= MAX_MESSAGE_LEN,
            "message trop gros ({} octets)",
            body.len()
        );
        self.send_raw(&body).await
    }

    /// Envoie des octets bruts préfixés par leur longueur (u32 big-endian).
    pub async fn send_raw(&mut self, body: &[u8]) -> anyhow::Result<()> {
        let mut plain = Vec::with_capacity(4 + body.len());
        plain.extend_from_slice(&(body.len() as u32).to_be_bytes());
        plain.extend_from_slice(body);

        let mut cipher = vec![0u8; NOISE_MAX_FRAME];
        for chunk in plain.chunks(MAX_CHUNK) {
            let len = self.noise.write_message(chunk, &mut cipher)?;
            write_frame(&mut self.stream, &cipher[..len]).await?;
        }
        Ok(())
    }

    /// Reçoit un message et le désérialise. La longueur annoncée est vérifiée
    /// avant toute allocation du corps.
    pub async fn recv<T: DeserializeOwned>(&mut self) -> anyhow::Result<T> {
        let body = self.recv_raw().await?;
        Ok(postcard::from_bytes(&body)?)
    }

    async fn recv_raw(&mut self) -> anyhow::Result<Vec<u8>> {
        let mut plain = self.read_noise_frame().await?;
        ensure!(plain.len() >= 4, "trame trop courte");
        let announced = u32::from_be_bytes([plain[0], plain[1], plain[2], plain[3]]) as usize;
        ensure!(
            announced <= MAX_MESSAGE_LEN,
            "message annoncé trop gros ({announced} octets)"
        );

        let expected = 4 + announced;
        while plain.len() < expected {
            plain.extend(self.read_noise_frame().await?);
        }
        if plain.len() != expected {
            bail!(
                "longueur incohérente : {} au lieu de {expected}",
                plain.len()
            );
        }
        Ok(plain.split_off(4))
    }

    async fn read_noise_frame(&mut self) -> anyhow::Result<Vec<u8>> {
        let frame = read_frame(&mut self.stream).await?;
        let mut plain = vec![0u8; frame.len()];
        let len = self.noise.read_message(&frame, &mut plain)?;
        plain.truncate(len);
        Ok(plain)
    }
}

async fn write_frame<S: AsyncWrite + Unpin>(stream: &mut S, bytes: &[u8]) -> anyhow::Result<()> {
    let len = u16::try_from(bytes.len()).context("trame Noise trop longue")?;
    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(bytes).await?;
    stream.flush().await?;
    Ok(())
}

async fn read_frame<S: AsyncRead + Unpin>(stream: &mut S) -> anyhow::Result<Vec<u8>> {
    let mut len = [0u8; 2];
    stream.read_exact(&mut len).await?;
    let mut frame = vec![0u8; u16::from_be_bytes(len) as usize];
    stream.read_exact(&mut frame).await?;
    Ok(frame)
}

/// Outils partagés par les tests : deux canaux reliés en mémoire.
#[cfg(test)]
pub(crate) mod test_support {
    use super::{Role, SecureChannel, handshake};
    use crate::identity::Identity;
    use tokio::io::{DuplexStream, duplex};

    /// Établit deux canaux Noise reliés par un tuyau en mémoire.
    pub(crate) async fn channel_pair() -> (SecureChannel<DuplexStream>, SecureChannel<DuplexStream>)
    {
        let (a, b) = duplex(1 << 20);
        let (ida, idb) = (Identity::generate().unwrap(), Identity::generate().unwrap());
        let (ca, cb) = tokio::join!(
            handshake(a, &ida, Role::Initiator),
            handshake(b, &idb, Role::Responder)
        );
        (ca.unwrap(), cb.unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::channel_pair;
    use super::*;

    #[tokio::test]
    async fn handshake_authenticates_both_static_keys() {
        let (a, b) = channel_pair().await;
        assert_eq!(a.handshake_hash, b.handshake_hash);
        assert_ne!(a.remote_static, b.remote_static);
    }

    #[tokio::test]
    async fn large_message_is_split_and_rebuilt() {
        let (mut a, mut b) = channel_pair().await;
        let payload = vec![7u8; 3 * MAX_CHUNK + 123];
        let sent = payload.clone();
        let (sent_res, received) = tokio::join!(a.send(&sent), b.recv::<Vec<u8>>());
        sent_res.unwrap();
        assert_eq!(received.unwrap(), payload);
    }

    #[tokio::test]
    async fn oversized_message_is_refused_on_send() {
        let (mut a, _b) = channel_pair().await;
        let payload = vec![0u8; MAX_MESSAGE_LEN + 1];
        assert!(a.send(&payload).await.is_err());
    }

    #[tokio::test]
    async fn oversized_announce_is_refused_on_receive() {
        let (mut a, mut b) = channel_pair().await;
        let mut cipher = vec![0u8; NOISE_MAX_FRAME];
        let forged = ((MAX_MESSAGE_LEN + 1) as u32).to_be_bytes();
        let len = a.noise.write_message(&forged, &mut cipher).unwrap();
        write_frame(&mut a.stream, &cipher[..len]).await.unwrap();
        assert!(b.recv::<Vec<u8>>().await.is_err());
    }
}
