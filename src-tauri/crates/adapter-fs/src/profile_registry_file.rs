//! `profiles.json`, à la racine du dossier de données : la liste des profils et le dernier
//! actif.

mod json;

use std::io::ErrorKind;
use std::path::PathBuf;

use app::ports::ProfileRegistryStore;
use app::{AppError, AppResult};
use domain::profile::ProfileRegistry;

use crate::profile_directories::write_atomically;

/// Extension de la copie gardée d'un registre illisible.
const CORRUPTED_EXTENSION: &str = "json.corrupted";

/// Registre des profils enregistré en JSON.
pub struct ProfileRegistryFile {
    path: PathBuf,
}

impl ProfileRegistryFile {
    /// Registre à ce chemin.
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Garde une copie du fichier illisible avant qu'il ne soit remplacé : les profils qu'il
    /// listait restent récupérables à la main (leurs dossiers ne sont pas touchés).
    fn keep_corrupted_copy(&self) {
        let copy = self.path.with_extension(CORRUPTED_EXTENSION);
        if let Err(error) = std::fs::copy(&self.path, &copy) {
            tracing::warn!(%error, "copie du registre des profils illisible impossible");
        }
    }
}

impl ProfileRegistryStore for ProfileRegistryFile {
    fn load(&self) -> AppResult<Option<ProfileRegistry>> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(AppError::file_system(error)),
        };
        let registry = serde_json::from_str(&text)
            .ok()
            .and_then(|value| json::registry_from_json(&value));

        if registry.is_none() {
            tracing::error!("profiles.json illisible : profil par défaut seul, copie gardée");
            self.keep_corrupted_copy();
        }
        Ok(registry)
    }

    fn save(&self, registry: &ProfileRegistry) -> AppResult<()> {
        let text = serde_json::to_string_pretty(&json::registry_to_json(registry))
            .map_err(AppError::file_system)?;
        write_atomically(&self.path, text.as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use domain::profile::{IconFormat, Profile, ProfileId};

    use super::*;

    fn file_in(directory: &tempfile::TempDir) -> ProfileRegistryFile {
        ProfileRegistryFile::new(directory.path().join("profiles.json"))
    }

    #[test]
    fn a_missing_file_has_no_registry() {
        let directory = tempfile::tempdir().unwrap();
        assert_eq!(file_in(&directory).load().unwrap(), None);
    }

    #[test]
    fn saves_and_reloads_the_registry() {
        let directory = tempfile::tempdir().unwrap();
        let file = file_in(&directory);
        let work = Profile {
            name: Some("Travail".to_owned()),
            icon: Some(IconFormat::Webp),
            password_hash: Some("$argon2id$v=19$m=19456,t=2,p=1$c2FsdA$aGFzaA".to_owned()),
            ..Profile::new(ProfileId::parse("a1b2c3").unwrap())
        };
        let registry = ProfileRegistry::new(vec![work.clone()], Some(work.id.clone()));

        file.save(&registry).unwrap();

        assert_eq!(file.load().unwrap(), Some(registry));
        assert!(!directory.path().join("profiles.json.tmp").exists());
    }

    #[test]
    fn a_corrupted_file_has_no_registry_and_is_kept_aside() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("profiles.json"), "{ nope").unwrap();

        assert_eq!(file_in(&directory).load().unwrap(), None);
        let copy = directory.path().join("profiles.json.corrupted");
        assert_eq!(std::fs::read_to_string(copy).unwrap(), "{ nope");
    }

    #[test]
    fn json_of_the_wrong_shape_is_corrupted_too() {
        let directory = tempfile::tempdir().unwrap();
        std::fs::write(directory.path().join("profiles.json"), "[1, 2]").unwrap();

        assert_eq!(file_in(&directory).load().unwrap(), None);
    }
}
