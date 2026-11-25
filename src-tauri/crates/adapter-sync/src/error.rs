//! Erreurs de la synchronisation réseau.

use std::io;
use std::net::IpAddr;

use app::AppError;

/// Erreur du protocole ou du moteur de synchronisation. Les données reçues étant non fiables,
/// une entrée réseau invalide donne une erreur, jamais une panique.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Adresse hors du réseau local : jamais composée, connexion refusée avant le handshake.
    #[error("address {0} is not on the local network")]
    NotLocal(IpAddr),
    /// Écoute impossible sur le port configuré (port pris, réseau indisponible).
    #[error("cannot listen on port {port}: {source}")]
    Listen {
        /// Port demandé.
        port: u16,
        /// Cause système.
        source: io::Error,
    },
    /// Aucune adresse de l'appareil n'a répondu.
    #[error("device unreachable")]
    Unreachable,
    /// Échec d'entrée-sortie sur une connexion établie.
    #[error("connection failed: {0}")]
    Io(#[from] io::Error),
    /// Échec du chiffrement Noise (handshake ou trame altérée).
    #[error("noise failure: {0}")]
    Noise(String),
    /// Message au-delà de la taille maximale.
    #[error("message too large: {0} bytes")]
    TooLarge(usize),
    /// Message illisible ou incohérent.
    #[error("malformed message")]
    Malformed,
    /// Message d'une version plus récente du protocole : ignoré.
    #[error("unknown message")]
    UnknownMessage,
    /// Message valide mais inattendu à ce stade du protocole.
    #[error("unexpected message")]
    Unexpected,
    /// Version du protocole incompatible.
    #[error("unsupported protocol version {0}")]
    Protocol(u32),
    /// Clé distante inconnue ou révoquée.
    #[error("unknown or revoked device")]
    Untrusted,
    /// Code d'appairage refusé.
    #[error("wrong pairing code")]
    WrongCode,
    /// L'appareil distant n'est pas (ou plus) en mode appairage.
    #[error("device not in pairing mode")]
    NotPairing,
    /// La clé de l'appareil joint ne correspond pas à l'empreinte du QR code.
    #[error("fingerprint mismatch")]
    FingerprintMismatch,
    /// Contenu de QR code invalide.
    #[error("invalid pairing QR code")]
    InvalidQr,
    /// Aucun appareil en mode appairage trouvé sur le réseau.
    #[error("no device in pairing mode found")]
    NoPairingDevice,
    /// Délai dépassé.
    #[error("timed out")]
    Timeout,
    /// Synchronisation en pause.
    #[error("synchronization paused")]
    Paused,
    /// Offre inconnue ou déjà traitée.
    #[error("unknown offer")]
    UnknownOffer,
    /// Échec de la découverte mDNS.
    #[error("discovery failure: {0}")]
    Discovery(String),
    /// Échec d'un port (stockage, profils, confiance).
    #[error(transparent)]
    App(#[from] AppError),
    /// Connexion fermée par l'appareil distant.
    #[error("connection closed")]
    Closed,
    /// Tâche interrompue (arrêt du moteur).
    #[error("engine stopped")]
    Stopped,
}

impl Error {
    /// Cause stable affichée par la barre d'état (clé traduite par l'interface).
    pub fn status_reason(&self) -> &'static str {
        match self {
            Self::Listen { .. } => "listen_failed",
            Self::Discovery(_) => "discovery_failed",
            Self::NotLocal(_) => "not_local",
            Self::Unreachable | Self::Timeout | Self::Io(_) | Self::Closed => "unreachable",
            Self::App(_) => "storage_failed",
            _ => "protocol_failed",
        }
    }
}

impl From<snow::Error> for Error {
    fn from(error: snow::Error) -> Self {
        Self::Noise(error.to_string())
    }
}

impl From<postcard::Error> for Error {
    fn from(_: postcard::Error) -> Self {
        Self::Malformed
    }
}

/// Résultat de la synchronisation réseau.
pub type Result<T> = std::result::Result<T, Error>;
