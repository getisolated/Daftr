//! Cas d'usage des réglages : chargement, patchs du renderer, enregistrement.

mod change;
pub mod json;

pub use change::SettingsChange;

use std::sync::{Arc, Mutex};

use domain::i18n::Language;
use domain::overlay::Position;
use domain::settings::{AppSettings, RawPosition, RawSettings, sanitize_settings};
use platform::sync::lock;

use crate::ports::SettingsStore;

/// Réglages courants, toujours valides, et leur persistance.
pub struct SettingsService {
    store: Arc<dyn SettingsStore>,
    current: Mutex<AppSettings>,
}

impl SettingsService {
    /// Service branché sur son stockage, avec les réglages par défaut tant que rien n'est
    /// chargé.
    pub fn new(store: Arc<dyn SettingsStore>) -> Self {
        Self {
            store,
            current: Mutex::new(AppSettings::default()),
        }
    }

    /// Charge les réglages ; un fichier absent ou illisible donne les défauts. Sans langue
    /// choisie (premier lancement, réglages antérieurs aux langues), la locale de l'OS
    /// décide, et ce choix est enregistré.
    pub fn load(&self, os_locale: Option<&str>) -> AppSettings {
        let raw = self.read_stored();
        let has_language = raw.language.as_deref().and_then(Language::parse).is_some();
        let mut settings = sanitize_settings(&raw, &AppSettings::default());

        *lock(&self.current) = settings.clone();
        if !has_language {
            settings.language = Language::from_locale(os_locale);
            self.commit(settings.clone());
        }
        settings
    }

    /// Réglages courants.
    pub fn current(&self) -> AppSettings {
        lock(&self.current).clone()
    }

    /// Réglages qui résulteraient du patch du renderer, sans les appliquer. La position de
    /// l'overlay appartient au processus principal : le renderer ne la change jamais.
    pub fn candidate(&self, mut patch: RawSettings) -> AppSettings {
        let before = self.current();
        patch.overlay_position = RawPosition::Absent;

        let merged = RawSettings::from_settings(&before).overridden_by(patch);
        sanitize_settings(&merged, &before)
    }

    /// Applique et enregistre des réglages valides ; rend l'avant/après pour que l'appelant
    /// applique les effets (raccourci, tray, démarrage…). Un échec d'écriture est journalisé
    /// : les réglages restent appliqués pour la session.
    pub fn commit(&self, next: AppSettings) -> SettingsChange {
        let before = std::mem::replace(&mut *lock(&self.current), next.clone());

        if let Err(error) = self.store.save(&next) {
            tracing::error!(%error, "réglages non enregistrés");
        }
        SettingsChange {
            before,
            after: next,
        }
    }

    /// Mémorise la position de l'overlay choisie par l'utilisateur.
    pub fn set_overlay_position(&self, position: Position) -> SettingsChange {
        let next = AppSettings {
            overlay_position: Some(position),
            ..self.current()
        };
        self.commit(next)
    }

    fn read_stored(&self) -> RawSettings {
        match self.store.load() {
            Ok(stored) => stored.unwrap_or_default(),
            Err(error) => {
                tracing::warn!(%error, "réglages illisibles, valeurs par défaut");
                RawSettings::default()
            }
        }
    }
}

#[cfg(test)]
mod tests;
