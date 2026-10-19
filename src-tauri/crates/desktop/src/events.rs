//! Événements poussés vers les fenêtres.

use app::collections::CollectionEvent;
use app::ports::CollectionEventSink;
use presentation::dto::CollectionEventDto;
use presentation::events::COLLECTIONS_EVENT;
use serde::Serialize;
use tauri::{AppHandle, Emitter, EventTarget};

use crate::windows::main_window::MAIN;

/// Envoie un événement à toutes les fenêtres ; un échec est journalisé (la fenêtre a pu
/// disparaître entre-temps).
pub fn emit_all<S: Serialize + Clone>(app: &AppHandle, event: &str, payload: S) {
    if let Err(error) = app.emit(event, payload) {
        tracing::warn!(%error, event, "événement non envoyé");
    }
}

/// Envoie un événement à une fenêtre.
pub fn emit_to<S: Serialize + Clone>(app: &AppHandle, label: &str, event: &str, payload: S) {
    if let Err(error) = app.emit_to(label, event, payload) {
        tracing::warn!(%error, event, label, "événement non envoyé");
    }
}

/// Envoie un événement à toutes les fenêtres sauf celle d'origine (l'autre fenêtre se met à
/// jour, celle qui a fait la demande l'est déjà).
pub fn emit_except<S: Serialize + Clone>(app: &AppHandle, sender: &str, event: &str, payload: S) {
    let others = |target: &EventTarget| match target {
        EventTarget::WebviewWindow { label }
        | EventTarget::Window { label }
        | EventTarget::Webview { label } => label != sender,
        EventTarget::Any | EventTarget::AnyLabel { .. } | EventTarget::App => false,
        // `EventTarget` est `non_exhaustive` : une cible inconnue ne reçoit rien
        _ => false,
    };
    if let Err(error) = app.emit_filter(event, payload, others) {
        tracing::warn!(%error, event, "événement non envoyé");
    }
}

/// Événements des collections vers la fenêtre principale.
pub struct WindowSink {
    app: AppHandle,
}

impl WindowSink {
    /// Destinataire branché sur l'application.
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl CollectionEventSink for WindowSink {
    fn publish(&self, event: CollectionEvent) {
        emit_to(
            &self.app,
            MAIN,
            COLLECTIONS_EVENT,
            CollectionEventDto::from(event),
        );
    }
}
