//! Empreinte du contenu d'un fichier.

use std::fmt;

use sha2::{Digest, Sha256};

/// SHA-256 du contenu d'un fichier, octet pour octet (fins de ligne comprises).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FileHash([u8; 32]);

impl FileHash {
    /// L'empreinte de `bytes`.
    pub fn of(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    /// Les 32 octets tels quels.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Les 32 octets.
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl fmt::Display for FileHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0
            .iter()
            .try_for_each(|byte| write!(formatter, "{byte:02x}"))
    }
}

impl fmt::Debug for FileHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "FileHash({:.8})", self.to_string())
    }
}
