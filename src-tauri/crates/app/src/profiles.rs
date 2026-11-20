//! Cas d'usage des profils : registre, nom, icône, mot de passe maître, suppression.
//!
//! Ouvrir la base d'un profil revient à l'assembleur (`desktop`) : ce service ne connaît que
//! le registre et les dossiers des profils.

pub mod transfer;

use std::sync::{Arc, Mutex};

use domain::profile::{
    IconFormat, Profile, ProfileId, ProfileRegistry, is_acceptable_password,
    normalize_profile_name, validate_icon,
};
use platform::sync::lock;

use crate::ports::{PasswordHasher, ProfileFiles, ProfileRegistryStore, SecretSource};
use crate::{AppError, AppResult, Refusal};

/// Nombre de caractères hexadécimaux d'un identifiant de profil généré.
const GENERATED_ID_LENGTH: usize = 12;
/// Tentatives pour tirer un identifiant libre.
const ID_ATTEMPTS: usize = 4;

/// Les ports dont dépend le service des profils.
pub struct ProfilePorts {
    /// Registre.
    pub registry: Arc<dyn ProfileRegistryStore>,
    /// Dossiers des profils.
    pub files: Arc<dyn ProfileFiles>,
    /// Hachage des mots de passe.
    pub hasher: Arc<dyn PasswordHasher>,
    /// Hasard des identifiants.
    pub secrets: Arc<dyn SecretSource>,
}

/// Profils : registre en mémoire, enregistré à chaque changement.
pub struct ProfilesService {
    ports: ProfilePorts,
    registry: Mutex<ProfileRegistry>,
}

impl ProfilesService {
    /// Service branché sur ses ports, avec le profil par défaut seul tant que rien n'est chargé.
    pub fn new(ports: ProfilePorts) -> Self {
        Self {
            ports,
            registry: Mutex::new(ProfileRegistry::default()),
        }
    }

    /// Charge le registre. Absent (premier lancement, données d'avant les profils) ou
    /// illisible, le profil par défaut seul est enregistré : ses données sont celles déjà en
    /// place.
    pub fn load(&self) -> ProfileRegistry {
        let stored = self.ports.registry.load().unwrap_or_else(|error| {
            tracing::warn!(%error, "registre des profils illisible, profil par défaut seul");
            None
        });
        let registry = stored.unwrap_or_else(|| {
            let fresh = ProfileRegistry::default();
            if let Err(error) = self.ports.registry.save(&fresh) {
                tracing::warn!(%error, "registre des profils non enregistré");
            }
            fresh
        });

        *lock(&self.registry) = registry.clone();
        registry
    }

    /// Le registre courant.
    pub fn registry(&self) -> ProfileRegistry {
        lock(&self.registry).clone()
    }

    /// Un profil, ou un refus s'il est inconnu.
    pub fn get(&self, id: &ProfileId) -> AppResult<Profile> {
        lock(&self.registry)
            .get(id)
            .cloned()
            .ok_or_else(|| Refusal::UnknownProfile.into())
    }

    /// Crée un profil (et son dossier) ; sa base est créée par l'assembleur.
    pub fn create(&self, name: Option<&str>) -> AppResult<Profile> {
        let id = self.free_id()?;
        let profile = Profile {
            name: normalize_profile_name(name),
            ..Profile::new(id)
        };

        self.ports.files.create_directory(&profile.id)?;
        self.update(|registry| {
            registry.upsert(profile.clone());
            Ok(profile.clone())
        })
    }

    /// Renomme un profil ; un nom vide le laisse sans nom.
    pub fn rename(&self, id: &ProfileId, name: Option<&str>) -> AppResult<Profile> {
        let name = normalize_profile_name(name);
        self.change(id, |profile| profile.name = name)
    }

    /// Remplace l'icône d'un profil (PNG ou WebP, déjà recadrée par le renderer).
    pub fn set_icon(&self, id: &ProfileId, bytes: &[u8]) -> AppResult<Profile> {
        let format = validate_icon(bytes).ok_or(Refusal::InvalidIcon)?;
        let previous = self.get(id)?.icon;

        self.ports.files.write_icon(id, format, bytes)?;
        if let Some(old) = previous.filter(|old| *old != format) {
            self.delete_icon_file(id, old);
        }
        self.change(id, |profile| profile.icon = Some(format))
    }

    /// Retire l'icône d'un profil.
    pub fn remove_icon(&self, id: &ProfileId) -> AppResult<Profile> {
        let previous = self.get(id)?.icon;
        let profile = self.change(id, |profile| profile.icon = None)?;

        if let Some(old) = previous {
            self.delete_icon_file(id, old);
        }
        Ok(profile)
    }

    /// L'icône d'un profil, `None` s'il n'en a pas.
    pub fn icon(&self, id: &ProfileId) -> AppResult<Option<(IconFormat, Vec<u8>)>> {
        let Some(format) = self.get(id)?.icon else {
            return Ok(None);
        };
        let bytes = self.ports.files.read_icon(id, format)?;
        Ok(Some((format, bytes)))
    }

    /// Vérifie le mot de passe d'un profil protégé ; un profil sans mot de passe s'ouvre
    /// toujours.
    pub fn check_password(&self, id: &ProfileId, password: Option<&str>) -> AppResult<()> {
        let Some(hash) = self.get(id)?.password_hash else {
            return Ok(());
        };
        let matches = password
            .filter(|password| is_acceptable_password(password))
            .is_some_and(|password| self.ports.hasher.verify(password, &hash));

        if matches {
            Ok(())
        } else {
            Err(Refusal::WrongPassword.into())
        }
    }

    /// Définit, change (`next`) ou retire (`next` absent ou vide) le mot de passe maître ; le
    /// mot de passe actuel (`current`) est exigé quand il y en a un.
    pub fn set_password(
        &self,
        id: &ProfileId,
        current: Option<&str>,
        next: Option<&str>,
    ) -> AppResult<Profile> {
        self.check_password(id, current)?;
        let hash = match next.filter(|next| !next.is_empty()) {
            Some(next) if !is_acceptable_password(next) => {
                return Err(Refusal::WrongPassword.into());
            }
            Some(next) => Some(self.ports.hasher.hash(next)?),
            None => None,
        };

        self.change(id, |profile| profile.password_hash = hash)
    }

    /// Supprime un profil : ses données partent à la corbeille ; un profil protégé demande son
    /// mot de passe. Le profil par défaut ne disparaît pas du registre : il repart vide, sans
    /// nom, icône ni mot de passe. Fermer le profil actif avant revient à l'appelant.
    pub fn delete(&self, id: &ProfileId, password: Option<&str>) -> AppResult<()> {
        self.check_password(id, password)?;

        self.ports.files.trash_directory(id)?;
        self.update(|registry| {
            if id.is_default() {
                registry.upsert(Profile::new(id.clone()));
            } else {
                registry.remove(id);
            }
            Ok(())
        })
    }

    /// Le profil à ouvrir quand `id` disparaît : un autre sans mot de passe de préférence (il
    /// s'ouvre directement), sinon le premier autre ; `None` s'il n'y en a pas.
    pub fn successor(&self, id: &ProfileId) -> Option<Profile> {
        let registry = lock(&self.registry);
        let mut others = registry
            .profiles()
            .iter()
            .filter(|profile| &profile.id != id);
        let first = others.clone().next().cloned();

        others
            .find(|profile| !profile.is_protected())
            .cloned()
            .or(first)
    }

    /// Retient le profil actif, rouvert au prochain lancement.
    pub fn set_last_active(&self, id: &ProfileId) -> AppResult<()> {
        self.update(|registry| {
            if registry.set_last_active(id) {
                Ok(())
            } else {
                Err(Refusal::UnknownProfile.into())
            }
        })
    }

    /// Modifie un profil connu et l'enregistre.
    fn change(&self, id: &ProfileId, edit: impl FnOnce(&mut Profile)) -> AppResult<Profile> {
        self.update(|registry| {
            let mut profile = registry.get(id).cloned().ok_or(Refusal::UnknownProfile)?;
            edit(&mut profile);
            registry.upsert(profile.clone());
            Ok(profile)
        })
    }

    /// Applique un changement au registre, l'enregistre, puis le garde en mémoire : un échec
    /// d'écriture laisse le registre tel qu'il était.
    fn update<T>(&self, edit: impl FnOnce(&mut ProfileRegistry) -> AppResult<T>) -> AppResult<T> {
        let mut current = lock(&self.registry);
        let mut next = current.clone();
        let result = edit(&mut next)?;

        self.ports.registry.save(&next)?;
        *current = next;
        Ok(result)
    }

    /// Un identifiant aléatoire qu'aucun profil ne porte.
    fn free_id(&self) -> AppResult<ProfileId> {
        for _ in 0..ID_ATTEMPTS {
            let token = self.ports.secrets.new_token()?;
            let suffix = token.get(token.len().saturating_sub(GENERATED_ID_LENGTH)..);
            let Some(id) = suffix.and_then(ProfileId::parse) else {
                continue;
            };
            if lock(&self.registry).get(&id).is_none() && !id.is_default() {
                return Ok(id);
            }
        }
        Err(AppError::storage("no free profile identifier"))
    }

    /// Supprime un ancien fichier d'icône ; un échec ne bloque rien (le registre ne le cite
    /// plus), il est seulement journalisé.
    fn delete_icon_file(&self, id: &ProfileId, format: IconFormat) {
        if let Err(error) = self.ports.files.delete_icon(id, format) {
            tracing::warn!(%error, profile = id.as_str(), "ancienne icône non supprimée");
        }
    }
}

#[cfg(test)]
mod tests;
