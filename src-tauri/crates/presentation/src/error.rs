//! Erreur rendue au renderer.

use app::AppError;
use serde::Serialize;
use ts_rs::TS;

/// Nature d'une erreur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum IpcErrorCode {
    /// Demande refusée par une règle ; le message est stable (le renderer le traduit).
    Refused,
    /// Échec de la base de données.
    Storage,
    /// Échec sur le disque ; le message est celui du système.
    FileSystem,
    /// Demande mal formée.
    InvalidInput,
}

/// Erreur d'une commande, sérialisée vers le renderer (qui en fait une `Error` de message
/// `message`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[ts(export)]
pub struct IpcError {
    /// Nature.
    pub code: IpcErrorCode,
    /// Message.
    pub message: String,
}

impl IpcError {
    /// Demande mal formée.
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self {
            code: IpcErrorCode::InvalidInput,
            message: message.into(),
        }
    }
}

impl From<AppError> for IpcError {
    fn from(error: AppError) -> Self {
        let code = match &error {
            AppError::Refused(_) => IpcErrorCode::Refused,
            AppError::Storage(_) => IpcErrorCode::Storage,
            AppError::FileSystem(_) => IpcErrorCode::FileSystem,
        };
        // Les refus sont attendus ; les échecs du stockage ou du disque méritent une trace
        if code != IpcErrorCode::Refused {
            tracing::warn!(%error, "commande en échec");
        }
        Self {
            code,
            message: error.to_string(),
        }
    }
}

impl From<app::Refusal> for IpcError {
    fn from(refusal: app::Refusal) -> Self {
        AppError::from(refusal).into()
    }
}

/// Résultat d'une commande.
pub type IpcResult<T> = Result<T, IpcError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_message_of_a_refusal() {
        let error = IpcError::from(app::Refusal::InvalidName);
        assert_eq!(
            error,
            IpcError {
                code: IpcErrorCode::Refused,
                message: "Invalid name".into()
            }
        );
    }

    #[test]
    fn serializes_for_the_renderer() {
        let error = IpcError::invalid_input("bad");
        let json = serde_json::to_value(&error).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "code": "invalidInput", "message": "bad" })
        );
    }
}
