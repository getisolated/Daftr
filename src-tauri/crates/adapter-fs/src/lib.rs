//! Adaptateurs du disque : réglages (`settings.json`), registre et dossiers des profils,
//! fichiers des collections, corbeille, exports, parcours et surveillance des collections.

mod disk;
mod export;
mod profile_directories;
mod profile_registry_file;
mod scan;
mod settings_file;
mod watcher;

pub use disk::DiskFiles;
pub use export::DiskExportWriter;
pub use profile_directories::{PROFILES_FOLDER, ProfileDirectories};
pub use profile_registry_file::ProfileRegistryFile;
pub use scan::{ScanResult, scan_folder};
pub use settings_file::SettingsFile;
pub use watcher::CollectionsWatcher;
