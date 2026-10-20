//! Lecture des lignes SQLite en entités du domaine.

use domain::collection::{Collection, CollectionKind};
use domain::folder::Folder;
use domain::note::Note;
use rusqlite::Row;
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ValueRef};

/// Une ligne de `notes`.
pub(crate) fn note(row: &Row<'_>) -> rusqlite::Result<Note> {
    Ok(Note {
        id: row.get("id")?,
        title: row.get("title")?,
        content: row.get("content")?,
        tags: row.get("tags")?,
        folder_id: row.get("folder_id")?,
        sort_order: row.get("sort_order")?,
        color: row.get("color")?,
        created_at: row
            .get::<_, Option<String>>("created_at")?
            .unwrap_or_default(),
        updated_at: row
            .get::<_, Option<String>>("updated_at")?
            .unwrap_or_default(),
    })
}

/// Une ligne de `folders`.
pub(crate) fn folder(row: &Row<'_>) -> rusqlite::Result<Folder> {
    Ok(Folder {
        id: row.get("id")?,
        name: row.get("name")?,
        parent_id: row.get("parent_id")?,
        is_default: row.get::<_, i64>("is_default")? == 1,
        sort_order: row.get("sort_order")?,
        color: row.get("color")?,
        created_at: row
            .get::<_, Option<String>>("created_at")?
            .unwrap_or_default(),
    })
}

/// Une ligne de `collections`.
pub(crate) fn collection(row: &Row<'_>) -> rusqlite::Result<Collection> {
    Ok(Collection {
        path: row.get("path")?,
        kind: row.get::<_, StoredKind>("kind")?.0,
        display_name: row.get("display_name")?,
        sort_order: row.get("sort_order")?,
        created_at: row
            .get::<_, Option<String>>("created_at")?
            .unwrap_or_default(),
    })
}

/// Colonne `kind` (contrainte `CHECK` côté base).
struct StoredKind(CollectionKind);

impl FromSql for StoredKind {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let text = value.as_str()?;
        CollectionKind::parse(text)
            .map(StoredKind)
            .ok_or(FromSqlError::InvalidType)
    }
}
