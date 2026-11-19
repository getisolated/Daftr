//! Canal chiffré : handshake Noise XX puis messages découpés en trames Noise.
//!
//! Le transport se partage entre une moitié de lecture et une moitié d'écriture (une session
//! lit et écrit en même temps) : l'état Noise est sous un verrou tenu le temps de chiffrer ou
//! de déchiffrer une trame, jamais pendant une attente réseau.

use std::sync::{Arc, Mutex};

use domain::sync::DeviceKey;
use platform::sync::lock;
use snow::{HandshakeState, TransportState};
use tokio::io::{AsyncRead, AsyncWrite, ReadHalf, WriteHalf};

use crate::error::{Error, Result};
use crate::framing::{
    MAX_CHUNK, NOISE_MAX_FRAME, Reassembly, prefix_message, read_frame, write_frame,
};
use crate::keys::DeviceIdentity;
use crate::wire::Message;

/// Motif et primitives Noise.
pub(crate) const NOISE_PARAMS: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
/// Prologue Noise : lie le handshake à ce protocole et à sa version.
const PROLOGUE: &[u8] = b"daftr-sync/1";

/// Rôle Noise : l'initiateur est l'appareil qui compose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    /// Côté qui compose.
    Initiator,
    /// Côté qui écoute.
    Responder,
}

/// Canal établi : la clé statique distante est authentifiée par le handshake.
pub(crate) struct Channel<S> {
    reader: ChannelReader<S>,
    writer: ChannelWriter<S>,
    /// Hash du handshake (32 octets) : lie l'appairage à cette connexion précise.
    pub handshake_hash: Vec<u8>,
    /// Clé statique distante.
    pub remote: DeviceKey,
}

/// Moitié de lecture d'un canal.
pub(crate) struct ChannelReader<S> {
    stream: ReadHalf<S>,
    noise: Arc<Mutex<TransportState>>,
}

/// Moitié d'écriture d'un canal.
pub(crate) struct ChannelWriter<S> {
    stream: WriteHalf<S>,
    noise: Arc<Mutex<TransportState>>,
}

/// Effectue le handshake Noise XX sur `stream` avec la clé statique de l'appareil.
pub(crate) async fn handshake<S>(
    mut stream: S,
    identity: &DeviceIdentity,
    role: Role,
) -> Result<Channel<S>>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let builder = snow::Builder::new(NOISE_PARAMS.parse()?)
        .prologue(PROLOGUE)?
        .local_private_key(identity.secret().as_bytes())?;
    let state = match role {
        Role::Initiator => builder.build_initiator()?,
        Role::Responder => builder.build_responder()?,
    };
    let state = drive(&mut stream, state).await?;
    into_channel(stream, state)
}

async fn drive<S>(stream: &mut S, mut state: HandshakeState) -> Result<HandshakeState>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let mut buffer = vec![0_u8; NOISE_MAX_FRAME];
    while !state.is_handshake_finished() {
        if state.is_my_turn() {
            let length = state.write_message(&[], &mut buffer)?;
            write_frame(stream, &buffer[..length]).await?;
        } else {
            let frame = read_frame(stream).await?;
            state.read_message(&frame, &mut buffer)?;
        }
    }
    Ok(state)
}

/// Le hash du handshake n'est plus lisible après `into_transport_mode` : on le copie avant.
fn into_channel<S>(stream: S, state: HandshakeState) -> Result<Channel<S>>
where
    S: AsyncRead + AsyncWrite,
{
    let handshake_hash = state.get_handshake_hash().to_vec();
    let remote = state
        .get_remote_static()
        .and_then(|key| <[u8; 32]>::try_from(key).ok())
        .map(DeviceKey::from_bytes)
        .ok_or(Error::Malformed)?;
    let noise = Arc::new(Mutex::new(state.into_transport_mode()?));
    let (read, write) = tokio::io::split(stream);

    Ok(Channel {
        reader: ChannelReader {
            stream: read,
            noise: Arc::clone(&noise),
        },
        writer: ChannelWriter {
            stream: write,
            noise,
        },
        handshake_hash,
        remote,
    })
}

impl<S: AsyncRead + AsyncWrite> Channel<S> {
    /// Envoie un message.
    pub(crate) async fn send(&mut self, message: &Message) -> Result<()> {
        self.writer.send(message).await
    }

    /// Reçoit un message.
    pub(crate) async fn recv(&mut self) -> Result<Message> {
        self.reader.recv().await
    }

    /// Sépare les moitiés de lecture et d'écriture.
    pub(crate) fn split(self) -> (ChannelReader<S>, ChannelWriter<S>) {
        (self.reader, self.writer)
    }
}

impl<S: AsyncRead + AsyncWrite> ChannelWriter<S> {
    /// Encode, découpe, chiffre et envoie un message ; refusé au-delà de la taille maximale.
    pub(crate) async fn send(&mut self, message: &Message) -> Result<()> {
        let plain = prefix_message(&message.encode()?)?;
        let mut cipher = vec![0_u8; NOISE_MAX_FRAME];
        for chunk in plain.chunks(MAX_CHUNK) {
            let length = lock(&self.noise).write_message(chunk, &mut cipher)?;
            write_frame(&mut self.stream, &cipher[..length]).await?;
        }
        Ok(())
    }
}

impl<S: AsyncRead + AsyncWrite> ChannelReader<S> {
    /// Reçoit, déchiffre, réassemble et décode un message.
    pub(crate) async fn recv(&mut self) -> Result<Message> {
        let mut message = Reassembly::start(&self.read_chunk().await?)?;
        while !message.is_complete() {
            message.push(&self.read_chunk().await?)?;
        }
        Message::decode(&message.into_body())
    }

    async fn read_chunk(&mut self) -> Result<Vec<u8>> {
        let frame = read_frame(&mut self.stream).await?;
        let mut plain = vec![0_u8; frame.len()];
        let length = lock(&self.noise).read_message(&frame, &mut plain)?;
        plain.truncate(length);
        Ok(plain)
    }
}

/// Deux canaux reliés en mémoire, pour les tests.
#[cfg(test)]
pub(crate) mod test_support {
    use super::*;
    use tokio::io::{DuplexStream, duplex};

    /// Envoie un message qui annonce `length` octets de corps sans les envoyer.
    pub(crate) async fn send_forged_length<S: AsyncRead + AsyncWrite>(
        writer: &mut ChannelWriter<S>,
        length: u32,
    ) {
        let mut cipher = vec![0_u8; NOISE_MAX_FRAME];
        let written = lock(&writer.noise)
            .write_message(&length.to_be_bytes(), &mut cipher)
            .unwrap();
        write_frame(&mut writer.stream, &cipher[..written])
            .await
            .unwrap();
    }

    /// Canaux de l'initiateur et du répondeur, avec leurs identités.
    pub(crate) async fn channel_pair(
        initiator: &DeviceIdentity,
        responder: &DeviceIdentity,
    ) -> (Channel<DuplexStream>, Channel<DuplexStream>) {
        let (a, b) = duplex(1 << 20);
        let (ca, cb) = tokio::join!(
            handshake(a, initiator, Role::Initiator),
            handshake(b, responder, Role::Responder)
        );
        (ca.unwrap(), cb.unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::channel_pair;
    use super::*;
    use crate::framing::MAX_MESSAGE_BYTES;

    fn identities() -> (DeviceIdentity, DeviceIdentity) {
        (
            DeviceIdentity::generate().unwrap(),
            DeviceIdentity::generate().unwrap(),
        )
    }

    #[tokio::test]
    async fn the_handshake_authenticates_both_static_keys() {
        let (a, b) = identities();
        let (ca, cb) = channel_pair(&a, &b).await;
        assert_eq!(ca.handshake_hash, cb.handshake_hash);
        assert_eq!(ca.handshake_hash.len(), 32);
        assert_eq!(ca.remote, b.key());
        assert_eq!(cb.remote, a.key());
    }

    #[tokio::test]
    async fn a_large_message_crosses_several_frames() {
        let (a, b) = identities();
        let (mut ca, mut cb) = channel_pair(&a, &b).await;
        let message = Message::Spake(vec![7; 3 * MAX_CHUNK + 123]);
        let (sent, received) = tokio::join!(ca.send(&message), cb.recv());
        sent.unwrap();
        assert_eq!(received.unwrap(), message);
    }

    #[tokio::test]
    async fn an_oversized_message_is_refused_on_send() {
        let (a, b) = identities();
        let (mut ca, _cb) = channel_pair(&a, &b).await;
        let message = Message::Spake(vec![0; MAX_MESSAGE_BYTES]);
        assert!(matches!(ca.send(&message).await, Err(Error::TooLarge(_))));
    }

    #[tokio::test]
    async fn an_oversized_announce_is_refused_on_receive() {
        let (a, b) = identities();
        let (ca, mut cb) = channel_pair(&a, &b).await;
        let (_, mut writer) = ca.split();
        let length = u32::try_from(MAX_MESSAGE_BYTES + 1).unwrap();
        super::test_support::send_forged_length(&mut writer, length).await;
        assert!(matches!(cb.recv().await, Err(Error::TooLarge(_))));
    }

    #[tokio::test]
    async fn split_halves_carry_messages_both_ways() {
        let (a, b) = identities();
        let (ca, cb) = channel_pair(&a, &b).await;
        let ((mut ra, mut wa), (mut rb, mut wb)) = (ca.split(), cb.split());
        let ping = Message::Spake(vec![1]);
        let pong = Message::Spake(vec![2]);
        wa.send(&ping).await.unwrap();
        wb.send(&pong).await.unwrap();
        assert_eq!(rb.recv().await.unwrap(), ping);
        assert_eq!(ra.recv().await.unwrap(), pong);
    }
}
