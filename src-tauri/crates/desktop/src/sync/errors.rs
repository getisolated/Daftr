//! Erreurs du moteur rendues au renderer : une clé stable par cause, traduite par
//! l'interface (`IpcErrorCode::Sync`).

use adapter_sync::Error;
use presentation::IpcError;

/// L'erreur du moteur pour le renderer ; une erreur d'un port garde sa nature (refus…).
pub fn ipc_error(error: Error) -> IpcError {
    match error {
        Error::App(error) => error.into(),
        other => {
            tracing::debug!(error = %other, "synchronisation en échec");
            IpcError::sync(reason(&other))
        }
    }
}

/// Clé stable de la cause.
fn reason(error: &Error) -> &'static str {
    match error {
        Error::NotLocal(_) => "not_local",
        Error::Listen { .. } => "listen_failed",
        Error::Unreachable | Error::Io(_) | Error::Closed => "unreachable",
        Error::Timeout => "timeout",
        Error::WrongCode => "wrong_code",
        Error::NotPairing => "not_pairing",
        Error::FingerprintMismatch => "fingerprint_mismatch",
        Error::InvalidQr => "invalid_qr",
        Error::NoPairingDevice => "no_pairing_device",
        Error::Paused => "paused",
        Error::UnknownOffer => "unknown_offer",
        Error::Untrusted => "untrusted",
        Error::Discovery(_) => "discovery_failed",
        Error::Protocol(_) => "protocol_version",
        Error::Stopped => "stopped",
        Error::App(_) => "storage_failed",
        Error::Noise(_)
        | Error::TooLarge(_)
        | Error::Malformed
        | Error::UnknownMessage
        | Error::Unexpected => "protocol_failed",
    }
}

#[cfg(test)]
mod tests {
    use app::{AppError, Refusal};
    use presentation::error::IpcErrorCode;

    use super::*;

    #[test]
    fn gives_a_stable_key_per_cause() {
        let wrong = ipc_error(Error::WrongCode);
        assert_eq!(wrong.code, IpcErrorCode::Sync);
        assert_eq!(wrong.message, "wrong_code");
        assert_eq!(ipc_error(Error::Unreachable).message, "unreachable");
    }

    #[test]
    fn keeps_the_refusals_of_the_ports() {
        let refused = ipc_error(Error::App(AppError::from(Refusal::UnknownProfile)));
        assert_eq!(refused.code, IpcErrorCode::Refused);
    }
}
