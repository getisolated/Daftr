//! Écriture des exports.

use std::io::Write;
use std::path::Path;

use app::ports::{ExportWriter, ZipEntry};
use app::{AppError, AppResult};
use zip::CompressionMethod;
use zip::write::{SimpleFileOptions, ZipWriter};

/// Exports écrits sur le disque.
#[derive(Debug, Default)]
pub struct DiskExportWriter;

impl ExportWriter for DiskExportWriter {
    fn write_text(&self, path: &Path, text: &str) -> AppResult<()> {
        std::fs::write(path, text).map_err(AppError::file_system)
    }

    fn write_zip(&self, path: &Path, entries: &[ZipEntry]) -> AppResult<()> {
        let file = std::fs::File::create(path).map_err(AppError::file_system)?;
        let mut archive = ZipWriter::new(file);
        let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        for entry in entries {
            archive
                .start_file(entry.path.as_str(), options)
                .map_err(AppError::file_system)?;
            archive
                .write_all(entry.content.as_bytes())
                .map_err(AppError::file_system)?;
        }
        archive.finish().map_err(AppError::file_system)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::*;

    #[test]
    fn writes_entries_that_read_back() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("export.zip");
        let entries = [
            ZipEntry {
                path: "Dossier/Été.md".into(),
                content: "# Été\n\nContenu".into(),
            },
            ZipEntry {
                path: "b.md".into(),
                content: String::new(),
            },
        ];

        DiskExportWriter.write_zip(&path, &entries).unwrap();

        let mut archive = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
        assert_eq!(archive.len(), 2);
        let mut text = String::new();
        archive
            .by_name("Dossier/Été.md")
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        assert_eq!(text, "# Été\n\nContenu");
    }
}
