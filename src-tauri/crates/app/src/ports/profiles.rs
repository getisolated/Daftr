//! Ports des profils : registre, dossiers, mots de passe, copie vers une autre base.

use domain::profile::{IconFormat, ProfileId, ProfileRegistry};
use domain::transfer::TransferBundle;

use crate::AppResult;

/// Persistance du registre des profils.
pub trait ProfileRegistryStore: Send + Sync {
    /// Le registre enregistré ; `None` au premier lancement, ou s'il est illisible (l'adaptateur
    /// le journalise et en garde une copie).
    fn load(&self) -> AppResult<Option<ProfileRegistry>>;
    /// Enregistre le registre d'un bloc : un arrêt brutal ne laisse jamais un fichier à moitié
    /// écrit.
    fn save(&self, registry: &ProfileRegistry) -> AppResult<()>;
}

/// Dossiers des profils sur le disque (le profil par défaut a la racine des données).
pub trait ProfileFiles: Send + Sync {
    /// Crée le dossier du profil.
    fn create_directory(&self, id: &ProfileId) -> AppResult<()>;
    /// Écrit l'icône du profil (format déjà vérifié).
    fn write_icon(&self, id: &ProfileId, format: IconFormat, bytes: &[u8]) -> AppResult<()>;
    /// Lit l'icône du profil.
    fn read_icon(&self, id: &ProfileId, format: IconFormat) -> AppResult<Vec<u8>>;
    /// Supprime l'icône du profil ; absente, rien à faire.
    fn delete_icon(&self, id: &ProfileId, format: IconFormat) -> AppResult<()>;
    /// Envoie le dossier du profil à la corbeille ; refusé pour le profil par défaut, dont le
    /// dossier est celui de toutes les données.
    fn trash_directory(&self, id: &ProfileId) -> AppResult<()>;
}

/// Hachage des mots de passe maîtres.
pub trait PasswordHasher: Send + Sync {
    /// Empreinte salée du mot de passe (chaîne PHC, qui porte l'algorithme et le sel).
    fn hash(&self, password: &str) -> AppResult<String>;
    /// Le mot de passe correspond à l'empreinte ; une empreinte illisible ne correspond à rien.
    fn verify(&self, password: &str, hash: &str) -> bool;
}

/// Écriture des éléments transférés dans la base d'un autre profil.
pub trait NoteImporter: Send + Sync {
    /// Crée dossiers et notes en une transaction : tout ou rien. Les notes sans dossier
    /// arrivent dans le dossier par défaut (créé s'il manque).
    fn import(&self, bundle: &TransferBundle) -> AppResult<()>;
}
