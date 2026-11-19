//! Trames sur le flux TCP et découpage des messages en trames Noise.
//!
//! Chaque trame est préfixée par sa longueur (`u16` big-endian) ; une trame Noise fait au plus
//! 65 535 octets, dont 16 de tag. Un message applicatif est préfixé par sa longueur (`u32`
//! big-endian), puis découpé en morceaux de 65 519 octets de clair, chiffrés un par un. La
//! longueur annoncée est vérifiée contre [`MAX_MESSAGE_BYTES`] avant toute allocation du corps.

use std::io;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::error::{Error, Result};

/// Taille maximale d'un message applicatif : au-delà, on rejette.
pub const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;

/// Taille maximale d'une trame Noise (chiffrée).
pub(crate) const NOISE_MAX_FRAME: usize = 65_535;
const NOISE_TAG_BYTES: usize = 16;
/// Clair maximal d'une trame Noise.
pub(crate) const MAX_CHUNK: usize = NOISE_MAX_FRAME - NOISE_TAG_BYTES;
const LENGTH_BYTES: usize = 4;

/// Écrit une trame préfixée par sa longueur.
pub(crate) async fn write_frame<S>(stream: &mut S, bytes: &[u8]) -> Result<()>
where
    S: AsyncWrite + Unpin,
{
    let length = u16::try_from(bytes.len()).map_err(|_| Error::TooLarge(bytes.len()))?;
    stream.write_all(&length.to_be_bytes()).await?;
    stream.write_all(bytes).await?;
    stream.flush().await?;
    Ok(())
}

/// Lit une trame préfixée par sa longueur ; [`Error::Closed`] si le flux se termine.
pub(crate) async fn read_frame<S>(stream: &mut S) -> Result<Vec<u8>>
where
    S: AsyncRead + Unpin,
{
    let mut length = [0_u8; 2];
    stream.read_exact(&mut length).await.map_err(closed)?;
    let mut frame = vec![0_u8; usize::from(u16::from_be_bytes(length))];
    stream.read_exact(&mut frame).await.map_err(closed)?;
    Ok(frame)
}

fn closed(error: io::Error) -> Error {
    match error.kind() {
        io::ErrorKind::UnexpectedEof => Error::Closed,
        _ => Error::Io(error),
    }
}

/// Le clair à chiffrer d'un message : sa longueur puis son corps, refusé au-delà de
/// [`MAX_MESSAGE_BYTES`].
pub(crate) fn prefix_message(body: &[u8]) -> Result<Vec<u8>> {
    if body.len() > MAX_MESSAGE_BYTES {
        return Err(Error::TooLarge(body.len()));
    }
    let length = u32::try_from(body.len()).map_err(|_| Error::TooLarge(body.len()))?;
    let mut plain = Vec::with_capacity(LENGTH_BYTES + body.len());
    plain.extend_from_slice(&length.to_be_bytes());
    plain.extend_from_slice(body);
    Ok(plain)
}

/// Reconstitution d'un message à partir de ses morceaux déchiffrés.
#[derive(Debug)]
pub(crate) struct Reassembly {
    expected: usize,
    body: Vec<u8>,
}

impl Reassembly {
    /// Commence avec le premier morceau : la longueur annoncée est vérifiée avant d'allouer.
    pub(crate) fn start(first: &[u8]) -> Result<Self> {
        let (length, rest) = first
            .split_first_chunk::<LENGTH_BYTES>()
            .ok_or(Error::Malformed)?;
        let expected =
            usize::try_from(u32::from_be_bytes(*length)).map_err(|_| Error::Malformed)?;
        if expected > MAX_MESSAGE_BYTES {
            return Err(Error::TooLarge(expected));
        }
        let mut reassembly = Self {
            expected,
            body: Vec::with_capacity(expected),
        };
        reassembly.push(rest)?;
        Ok(reassembly)
    }

    /// Ajoute un morceau ; refuse un message plus long qu'annoncé.
    pub(crate) fn push(&mut self, chunk: &[u8]) -> Result<()> {
        if self.body.len() + chunk.len() > self.expected {
            return Err(Error::Malformed);
        }
        self.body.extend_from_slice(chunk);
        Ok(())
    }

    /// Le message est complet.
    pub(crate) fn is_complete(&self) -> bool {
        self.body.len() == self.expected
    }

    /// Le corps du message.
    pub(crate) fn into_body(self) -> Vec<u8> {
        self.body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_message_is_rebuilt_from_its_chunks() {
        let body: Vec<u8> = (0..3 * MAX_CHUNK + 17).map(|i| (i % 251) as u8).collect();
        let plain = prefix_message(&body).unwrap();
        let mut chunks = plain.chunks(MAX_CHUNK);

        let mut reassembly = Reassembly::start(chunks.next().unwrap()).unwrap();
        for chunk in chunks {
            assert!(!reassembly.is_complete());
            reassembly.push(chunk).unwrap();
        }
        assert!(reassembly.is_complete());
        assert_eq!(reassembly.into_body(), body);
    }

    #[test]
    fn an_oversized_message_is_refused_before_allocation() {
        let forged = u32::try_from(MAX_MESSAGE_BYTES + 1).unwrap().to_be_bytes();
        assert!(matches!(
            Reassembly::start(&forged),
            Err(Error::TooLarge(_))
        ));
        assert!(matches!(
            prefix_message(&vec![0; MAX_MESSAGE_BYTES + 1]),
            Err(Error::TooLarge(_))
        ));
    }

    #[test]
    fn inconsistent_lengths_are_refused() {
        assert!(matches!(Reassembly::start(&[0, 0]), Err(Error::Malformed)));
        let mut reassembly = Reassembly::start(&[0, 0, 0, 2, 1]).unwrap();
        assert!(matches!(reassembly.push(&[2, 3]), Err(Error::Malformed)));
    }

    #[tokio::test]
    async fn frames_round_trip_and_eof_means_closed() {
        let (mut a, mut b) = tokio::io::duplex(1024);
        write_frame(&mut a, b"bonjour").await.unwrap();
        assert_eq!(read_frame(&mut b).await.unwrap(), b"bonjour");
        drop(a);
        assert!(matches!(read_frame(&mut b).await, Err(Error::Closed)));
    }
}
