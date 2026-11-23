//! Une collection passe dans un autre profil : seul son enregistrement bouge, ses fichiers
//! restent où ils sont sur le disque.

use super::CollectionsService;
use crate::AppResult;
use crate::ports::CollectionStore;

impl CollectionsService {
    /// Enregistre la collection `root` dans `target` (déjà présente là-bas, sans tenir compte
    /// de la casse : rien à ajouter), puis la retire d'ici. `false` pour une collection
    /// inconnue.
    pub fn transfer_to(&self, root: &str, target: &dyn CollectionStore) -> AppResult<bool> {
        let Some(collection) = self.store.get(root)? else {
            return Ok(false);
        };
        let lower = collection.path.to_lowercase();
        let already_there = target
            .all()?
            .iter()
            .any(|existing| existing.path.to_lowercase() == lower);

        if !already_there {
            target.add(&collection.path, collection.kind, &collection.display_name)?;
        }
        self.remove(&collection.path)?;
        Ok(true)
    }
}
