//! Messages applicatifs échangés dans le canal Noise (encodés en `postcard`).

use serde::{Deserialize, Serialize};

/// Type de session annoncé par le premier message (§7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionKind {
    /// Appairage par code.
    Pair,
    /// Synchronisation entre appareils déjà liés (hors périmètre du spike).
    Sync,
}

/// Tous les messages du spike.
#[derive(Debug, Serialize, Deserialize)]
pub enum Message {
    /// Premier message : type de session.
    Session(SessionKind),
    /// Message SPAKE2 (32 octets + octet de groupe).
    Spake(Vec<u8>),
    /// Confirmation de clé : MAC sur le hash du handshake Noise.
    Confirm(Vec<u8>),
    /// Code refusé par l'appareil qui écoute.
    Refused,
    /// State vector Yjs (v1).
    StateVector(Vec<u8>),
    /// Diff Yjs (update v1) calculé depuis le state vector distant.
    Diff(Vec<u8>),
    /// Contenu final, pour vérifier la convergence.
    Content(String),
}
