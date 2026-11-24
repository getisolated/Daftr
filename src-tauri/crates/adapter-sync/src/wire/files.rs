//! Messages de la synchronisation des collections : offre, manifeste, fichiers par morceaux.
//!
//! Ils ne partent qu'aux appareils qui annoncent [`FEATURE_COLLECTIONS`] dans `Hello` : une
//! version antérieure décode un indice de variante inconnu comme une erreur et fermerait la
//! session.

use app::collection_sync::{FileEntry, FileHash, SyncPath};
use serde::{Deserialize, Serialize};

/// Bit de `Hello::features` : l'appareil comprend les messages de collections, rattachées à
/// l'appareil et non plus à l'espace d'un profil. Le bit 0 de la première version, abandonnée
/// avant toute publication, n'est plus annoncé : ses offres n'avaient pas le même format.
pub(crate) const FEATURE_COLLECTIONS: u8 = 1 << 1;
/// Bit de `Hello::features` : l'appareil comprend `ProfileInfo` (nom et icône d'un profil
/// partagé, pour en créer le clone).
pub(crate) const FEATURE_PROFILE_INFO: u8 = 1 << 2;
/// Bit de `Hello::features` : l'appareil comprend `Removed` (« je vous ai retiré de mes
/// appareils »). Annoncé pour que le message ne parte qu'à un appareil qui le lit.
pub(crate) const FEATURE_REMOVED: u8 = 1 << 3;

/// Lignes de manifeste au plus par message.
pub(crate) const MANIFEST_PART: usize = 500;
/// Collections au plus dans une offre.
pub(crate) const MAX_OFFERED: usize = 64;
/// Longueur maximale du nom d'une collection proposée, en caractères.
const MAX_NAME_CHARS: usize = 128;
/// Longueur maximale d'un nom de profil reçu, en caractères.
const MAX_PROFILE_NAME_CHARS: usize = domain::profile::MAX_PROFILE_NAME_LENGTH;
/// Longueur maximale d'un identifiant de profil reçu, en caractères.
const MAX_PROFILE_ID_CHARS: usize = domain::profile::MAX_PROFILE_ID_LENGTH;

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
    /// Espace du profil qui porte la collection, s'il est synchronisé avec le destinataire.
    pub space: Option<[u8; 32]>,
    /// Identifiant du profil qui porte la collection chez l'émetteur.
    pub profile_id: String,
    /// Nom de ce profil chez l'émetteur (vide : profil sans nom).
    pub profile_name: String,
    /// Identifiant du partage.
    pub share: [u8; 16],
    /// Nom proposé.
    pub name: String,
    /// Nombre de fichiers `.md`.
    pub files: u64,
    /// Volume des fichiers, en octets.
    pub bytes: u64,
}

fn clean(text: &str, max: usize) -> String {
    let cleaned: String = text.chars().filter(|c| !c.is_control()).take(max).collect();
    cleaned.trim().to_owned()
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

    /// Le nom du profil d'origine reçu, assaini.
    pub(crate) fn clean_profile_name(&self) -> String {
        clean(&self.profile_name, MAX_PROFILE_NAME_CHARS)
    }

    /// L'identifiant du profil d'origine reçu, assaini : il n'est jamais utilisé comme chemin.
    pub(crate) fn clean_profile_id(&self) -> String {
        clean(&self.profile_id, MAX_PROFILE_ID_CHARS)
    }
}

/// Nom et icône d'un profil partagé, envoyés par celui qui le partage avant son contenu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ProfileInfoEntry {
    /// Espace du profil.
    pub space: [u8; 32],
    /// Nom du profil, absent pour un profil sans nom.
    pub name: Option<String>,
    /// Image de l'icône (PNG ou WebP), absente sans icône.
    pub icon: Option<Vec<u8>>,
}

impl ProfileInfoEntry {
    /// Le nom reçu, assaini ; `None` s'il est vide.
    pub(crate) fn clean_name(&self) -> Option<String> {
        let name = clean(self.name.as_deref()?, MAX_PROFILE_NAME_CHARS);
        (!name.is_empty()).then_some(name)
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
