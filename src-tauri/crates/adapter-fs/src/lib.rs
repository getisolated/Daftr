//! Adaptateurs du disque : réglages (`settings.json`), registre et dossiers des profils,
//! fichiers des collections, corbeille, exports, parcours et surveillance des collections,
//! fichiers de la synchronisation (clé de l'appareil, appareils de confiance, cache des espaces).

mod disk;
mod export;
mod profile_directories;
mod profile_registry_file;
mod scan;
mod settings_file;
mod sync_files;
mod watcher;

pub use disk::DiskFiles;
pub use export::DiskExportWriter;
pub use profile_directories::{DATABASE_FILE, PROFILES_FOLDER, ProfileDirectories};
pub use profile_registry_file::ProfileRegistryFile;
pub use scan::{ScanResult, scan_folder};
pub use settings_file::SettingsFile;
pub use sync_files::{SYNC_FOLDER, SpaceIndexEntry, SpaceIndexFile, TrustFiles};
pub use watcher::CollectionsWatcher;
