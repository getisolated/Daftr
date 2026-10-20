//! `settings.json`, dans le dossier de données de l'application.

use std::io::ErrorKind;
use std::path::PathBuf;

use app::ports::SettingsStore;
use app::settings::json::{raw_from_json, settings_to_json};
use app::{AppError, AppResult};
use domain::settings::{AppSettings, RawSettings};

/// Réglages enregistrés en JSON (indentation de deux espaces, comme la version Electron).
pub struct SettingsFile {
    path: PathBuf,
}

impl SettingsFile {
    /// Fichier de réglages à ce chemin.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl SettingsStore for SettingsFile {
    fn load(&self) -> AppResult<Option<RawSettings>> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(AppError::file_system(error)),
        };
        match serde_json::from_str(&text) {
            Ok(value) => Ok(Some(raw_from_json(&value))),
            Err(error) => {
                tracing::warn!(%error, "settings.json corrompu, valeurs par défaut");
                Ok(None)
            }
        }
    }

    /// Écrit dans un fichier voisin puis le renomme : un arrêt brutal ne laisse jamais un
    /// fichier à moitié écrit.
    fn save(&self, settings: &AppSettings) -> AppResult<()> {
        let text = serde_json::to_string_pretty(&settings_to_json(settings))
            .map_err(AppError::file_system)?;
        let temporary = self.path.with_extension("json.tmp");

        std::fs::write(&temporary, text).map_err(AppError::file_system)?;
        std::fs::rename(&temporary, &self.path).map_err(AppError::file_system)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file_in(directory: &tempfile::TempDir) -> SettingsFile {
        SettingsFile::new(directory.path().join("settings.json"))
    }

    #[test]
    fn a_missing_file_has_no_settings() {
        let directory = tempfile::tempdir().unwrap();
        assert_eq!(file_in(&directory).load().unwrap(), None);
    }

    #[test]
    fn a_corrupted_file_has_no_settings() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("settings.json"), "{ nope").unwrap();

        assert_eq!(file_in(&directory).load().unwrap(), None);
    }

    #[test]
    fn saves_and_reloads_the_settings() {
        let directory = tempfile::tempdir().unwrap();
        let file = file_in(&directory);
        let settings = AppSettings {
            word_wrap: false,
            ..AppSettings::default()
        };

        file.save(&settings).unwrap();
        let raw = file.load().unwrap().unwrap();

        let reloaded = domain::settings::sanitize_settings(&raw, &AppSettings::default());
        assert_eq!(reloaded, settings);
    }

    #[test]
    fn reads_a_file_written_by_the_electron_version() {
        let directory = tempfile::tempdir().unwrap();
        let electron = r#"{ "language": "fr", "overlayWidth": 300, "theme": "system",
            "overlayPosition": { "x": 1500, "y": 700 }, "unknownField": true }"#;
        std::fs::write(directory.path().join("settings.json"), electron).unwrap();

        let raw = file_in(&directory).load().unwrap().unwrap();

        assert_eq!(raw.language.as_deref(), Some("fr"));
        assert_eq!(raw.overlay_width, Some(300.0));
    }
}
