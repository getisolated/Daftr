//! Messages de la synchronisation des collections : offre, manifeste, fichiers par morceaux.
//!
//! Ils ne partent qu'aux appareils qui annoncent [`FEATURE_COLLECTIONS`] dans `Hello` : une
//! version antérieure décode un indice de variante inconnu comme une erreur et fermerait la
//! session.

use app::collection_sync::{FileEntry, FileHash, SyncPath};
use serde::{Deserialize, Serialize};

/// Bit de `Hello::features` : l'appareil comprend les messages de collections.
pub(crate) const FEATURE_COLLECTIONS: u8 = 1;

/// Lignes de manifeste au plus par message.
pub(crate) const MANIFEST_PART: usize = 500;
/// Collections au plus dans une offre.
pub(crate) const MAX_OFFERED: usize = 64;
/// Longueur maximale du nom d'une collection proposée, en caractères.
const MAX_NAME_CHARS: usize = 128;

/// Un fichier dans un manifeste : le chemin et les empreintes en clair, validés à la réception.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct WireEntry {
    /// Chemin relatif, séparateurs `/`.
    pub path: String,
    /// Empreinte actuelle, absente pour une suppression.
    pub hash: Option<[u8; 32]>,
    /// Taille actuelle.
    pub size: u64,
    /// Dernière version échangée avec le destinataire.
    pub base: Option<[u8; 32]>,
}

/// Une collection proposée.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct OfferedCollection {
    /// Espace du profil qui porte la collection.
    pub space: [u8; 32],
    /// Identifiant du partage.
    pub share: [u8; 16],
    /// Nom proposé.
    pub name: String,
    /// Nombre de fichiers `.md`.
    pub files: u64,
    /// Volume des fichiers, en octets.
    pub bytes: u64,
}

impl OfferedCollection {
    /// Le nom reçu, assaini (caractères de contrôle retirés, longueur bornée).
    pub(crate) fn clean_name(&self) -> String {
        let name: String = self
            .name
            .chars()
            .filter(|c| !c.is_control() && !matches!(c, '/' | '\\'))
            .take(MAX_NAME_CHARS)
            .collect();
        name.trim().to_owned()
    }
}

/// La ligne à envoyer.
pub(crate) fn wire_entry(entry: &FileEntry) -> WireEntry {
    WireEntry {
        path: entry.path.as_str().to_owned(),
        hash: entry.hash.map(|hash| *hash.as_bytes()),
        size: entry.size,
        base: entry.base.map(|hash| *hash.as_bytes()),
    }
}

/// La ligne reçue, `None` si son chemin est refusé (hors collection, nom invalide, dossier
/// exclu) : le fichier n'est tout simplement pas synchronisé ici.
pub(crate) fn file_entry(wire: WireEntry, excludes: &[String]) -> Option<FileEntry> {
    let path = SyncPath::parse(&wire.path, excludes).ok()?;
    Some(FileEntry {
        path,
        hash: wire.hash.map(FileHash::from_bytes),
        size: wire.size,
        base: wire.base.map(FileHash::from_bytes),
    })
}
