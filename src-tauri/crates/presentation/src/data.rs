//! Données du profil actif (notes, dossiers, collections), remplacées d'un bloc quand
//! l'utilisateur change de profil.
//!
//! Chaque commande prend un instantané (`Arc`) des services : celle qui tourne pendant un
//! changement de profil finit sur l'ancienne base, fermée quand le dernier instantané tombe.

use std::sync::{Arc, Mutex};

use app::Refusal;
use app::collections::CollectionsService;
use app::folders::FoldersService;
use app::notes::NotesService;
use app::ports::{FolderStore, NoteStore};
use domain::profile::ProfileId;
use platform::sync::lock;

use crate::IpcResult;

/// Les services d'une base de profil.
pub struct DataServices {
    /// Notes.
    pub notes: Arc<NotesService>,
    /// Dossiers.
    pub folders: Arc<FoldersService>,
    /// Collections (et leur surveillance, arrêtée avec elles).
    pub collections: CollectionsService,
    /// Stockage des notes, pour un transfert vers un autre profil.
    pub note_store: Arc<dyn NoteStore>,
    /// Stockage des dossiers, pour un transfert vers un autre profil.
    pub folder_store: Arc<dyn FolderStore>,
}

struct Active {
    profile: ProfileId,
    /// `None` tant que le profil, protégé, attend son mot de passe.
    data: Option<Arc<DataServices>>,
}

/// Le profil actif et ses données.
pub struct ActiveData {
    active: Mutex<Active>,
}

impl Default for ActiveData {
    /// Le profil par défaut, pas encore ouvert.
    fn default() -> Self {
        Self {
            active: Mutex::new(Active {
                profile: ProfileId::default_profile(),
                data: None,
            }),
        }
    }
}

impl ActiveData {
    /// Le profil actif.
    pub fn profile(&self) -> ProfileId {
        lock(&self.active).profile.clone()
    }

    /// Les données du profil actif, ou un refus tant qu'il est verrouillé.
    pub fn get(&self) -> IpcResult<Arc<DataServices>> {
        self.current().ok_or_else(|| Refusal::ProfileLocked.into())
    }

    /// Les données du profil actif, `None` tant qu'il est verrouillé.
    pub fn current(&self) -> Option<Arc<DataServices>> {
        lock(&self.active).data.clone()
    }

    /// Le profil actif attend son mot de passe.
    pub fn is_locked(&self) -> bool {
        lock(&self.active).data.is_none()
    }

    /// Change de profil actif (`data` absent : verrouillé) ; rend les données d'avant.
    pub fn set(&self, profile: ProfileId, data: Option<DataServices>) -> Option<Arc<DataServices>> {
        let mut active = lock(&self.active);
        active.profile = profile;
        std::mem::replace(&mut active.data, data.map(Arc::new))
    }
}
