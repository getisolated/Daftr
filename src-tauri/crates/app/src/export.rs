//! Cas d'usage de l'export des notes vers le disque.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::export::{ExportKind, ExportedNote, UNTITLED, safe_file_name, zip_paths};

use crate::ports::{ExportWriter, ZipEntry};
use crate::{AppResult, Refusal};

/// Contenu à exporter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportPayload {
    /// Une note, en markdown ou en HTML.
    Text(String),
    /// Les notes d'un dossier, pour une archive.
    Notes(Vec<ExportedNote>),
}

/// Export des notes.
pub struct ExportService {
    writer: Arc<dyn ExportWriter>,
}

impl ExportService {
    /// Service branché sur son écrivain.
    pub fn new(writer: Arc<dyn ExportWriter>) -> Self {
        Self { writer }
    }

    /// Nom de fichier proposé pour un export (titre rendu sûr, extension du format).
    pub fn suggested_file_name(kind: ExportKind, title: &str) -> String {
        format!("{}.{}", safe_file_name(title, UNTITLED), kind.extension())
    }

    /// Écrit l'export à l'emplacement choisi par l'utilisateur ; rend ce chemin. Une archive
    /// attend des notes, les autres formats un texte.
    pub fn save(
        &self,
        kind: ExportKind,
        path: &Path,
        payload: ExportPayload,
    ) -> AppResult<PathBuf> {
        match (kind, payload) {
            (ExportKind::Zip, ExportPayload::Notes(notes)) => {
                self.writer.write_zip(path, &zip_entries(notes))?
            }
            (ExportKind::Markdown | ExportKind::Html, ExportPayload::Text(text)) => {
                self.writer.write_text(path, &text)?;
            }
            (ExportKind::Zip, ExportPayload::Text(_))
            | (ExportKind::Markdown | ExportKind::Html, ExportPayload::Notes(_)) => {
                return Err(Refusal::InvalidExport.into());
            }
        }
        Ok(path.to_path_buf())
    }
}

fn zip_entries(notes: Vec<ExportedNote>) -> Vec<ZipEntry> {
    let paths = zip_paths(&notes);

    notes
        .into_iter()
        .zip(paths)
        .map(|(note, path)| ZipEntry {
            path,
            content: note.content,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::RecordingWriter;

    fn note(path: &str) -> ExportedNote {
        ExportedNote {
            path: path.to_owned(),
            content: format!("# {path}"),
        }
    }

    #[test]
    fn suggests_a_safe_file_name() {
        assert_eq!(
            ExportService::suggested_file_name(ExportKind::Html, "a/b"),
            "a-b.html"
        );
        assert_eq!(
            ExportService::suggested_file_name(ExportKind::Markdown, " "),
            "Untitled.md"
        );
    }

    #[test]
    fn writes_a_text_export() {
        let writer = Arc::new(RecordingWriter::default());
        let service = ExportService::new(writer.clone());

        let written = service.save(
            ExportKind::Markdown,
            Path::new("n.md"),
            ExportPayload::Text("x".into()),
        );

        assert_eq!(written.unwrap(), PathBuf::from("n.md"));
        assert_eq!(writer.texts(), [(PathBuf::from("n.md"), "x".to_owned())]);
    }

    #[test]
    fn writes_the_notes_of_a_folder_with_unique_paths() {
        let writer = Arc::new(RecordingWriter::default());
        let service = ExportService::new(writer.clone());
        let notes = vec![note("Projet/Idées"), note("Projet/idées")];

        service
            .save(
                ExportKind::Zip,
                Path::new("d.zip"),
                ExportPayload::Notes(notes),
            )
            .unwrap();

        let paths: Vec<String> = writer
            .zip_entries()
            .into_iter()
            .map(|entry| entry.path)
            .collect();
        assert_eq!(paths, ["Projet/Idées.md", "Projet/idées (2).md"]);
    }

    #[test]
    fn refuses_a_payload_that_does_not_match_the_format() {
        let service = ExportService::new(Arc::new(RecordingWriter::default()));

        let zip_of_text = service.save(
            ExportKind::Zip,
            Path::new("d.zip"),
            ExportPayload::Text(String::new()),
        );
        let html_of_notes = service.save(
            ExportKind::Html,
            Path::new("n.html"),
            ExportPayload::Notes(vec![]),
        );

        assert!(matches!(
            zip_of_text,
            Err(crate::AppError::Refused(Refusal::InvalidExport))
        ));
        assert!(matches!(
            html_of_notes,
            Err(crate::AppError::Refused(Refusal::InvalidExport))
        ));
    }
}
