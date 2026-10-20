//! Adaptateurs du disque : réglages (`settings.json`), fichiers des collections, corbeille,
//! exports, parcours et surveillance des collections.

mod disk;
mod export;
mod scan;
mod settings_file;
mod watcher;

pub use disk::DiskFiles;
pub use export::DiskExportWriter;
pub use scan::{ScanResult, scan_folder};
pub use settings_file::SettingsFile;
pub use watcher::CollectionsWatcher;
