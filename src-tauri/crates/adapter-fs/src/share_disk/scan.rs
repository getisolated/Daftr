//! Parcours d'une collection partagée : les `.md` seulement, sans suivre de lien symbolique.

use std::fs::DirEntry;
use std::path::Path;

use app::collection_sync::{Ignored, IgnoredReason, MAX_FILE_BYTES, SyncPath};
use app::ports::collection_sync::{FileStat, LocalScan, ScannedFile};
use domain::collection::{MAX_SCAN_DEPTH, MAX_SCANNED_FILES, is_excluded_folder, is_markdown_file};

use super::mtime_nanos;

/// Les fichiers markdown de `root` hors dossiers exclus, à toute profondeur. Les dossiers
/// illisibles sont sautés ; les liens symboliques ne sont ni suivis ni listés.
pub(super) fn scan(root: &Path, excludes: &[String]) -> LocalScan {
    let mut walk = Walk {
        excludes,
        result: LocalScan::default(),
    };
    walk.visit(root, "", 0);
    walk.result
}

struct Walk<'a> {
    excludes: &'a [String],
    result: LocalScan,
}

impl Walk<'_> {
    fn visit(&mut self, directory: &Path, relative: &str, depth: usize) {
        if depth > MAX_SCAN_DEPTH {
            self.result.truncated = true;
            return;
        }
        let Ok(entries) = std::fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            if self.result.files.len() >= MAX_SCANNED_FILES {
                self.result.truncated = true;
                return;
            }
            self.visit_entry(&entry, relative, depth);
        }
    }

    fn visit_entry(&mut self, entry: &DirEntry, relative: &str, depth: usize) {
        let Ok(file_type) = entry.file_type() else {
            return;
        };
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            return;
        };
        let path = if relative.is_empty() {
            name.clone()
        } else {
            format!("{relative}/{name}")
        };
        if file_type.is_dir() {
            if !is_excluded_folder(&name, self.excludes) {
                self.visit(&entry.path(), &path, depth + 1);
            }
        } else if file_type.is_file() && is_markdown_file(&name) {
            self.add_file(entry, path);
        }
    }

    fn add_file(&mut self, entry: &DirEntry, path: String) {
        let Ok(metadata) = entry.metadata() else {
            return self.ignore(path, IgnoredReason::Unreadable);
        };
        let Ok(valid) = SyncPath::parse(&path, self.excludes) else {
            return self.ignore(path, IgnoredReason::InvalidPath);
        };
        if metadata.len() > MAX_FILE_BYTES {
            return self.ignore(path, IgnoredReason::TooLarge);
        }
        let stat = FileStat {
            size: metadata.len(),
            mtime: mtime_nanos(&metadata),
        };
        self.result.files.push(ScannedFile { path: valid, stat });
    }

    fn ignore(&mut self, path: String, reason: IgnoredReason) {
        self.result.ignored.push(Ignored { path, reason });
    }
}
