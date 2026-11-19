//! Document CRDT d'une note (`note:<uuid>`) : écriture des modifications locales et
//! projection déterministe vers la ligne SQLite.
//!
//! Racines : `content` (`Y.Text`), `meta` (`Y.Map` : `tags`, `folder`, `sort_order`, `color`,
//! `created_at`, `updated_at`, `deleted_at`, `deleted_sv`) et `edits` (`Y.Map` : identifiant
//! client → horloge de sa dernière modification du contenu, qui départage suppression et
//! modification concurrentes).

mod deletion;
#[cfg(test)]
mod tests;

use domain::color::normalize_color;
use domain::note::extract_title;
use domain::sync::{DeviceKey, is_purgeable};
use domain::tags::normalize_tags;
use domain::uuid::Uuid;
use yrs::{Doc, GetString, MapRef, ReadTxn, Text, TextRef, Transact, TransactionMut};

use crate::error::SyncResult;
use crate::sync::client::new_doc;
use crate::sync::codec::{SyncDoc, Update, sealed};
use crate::sync::text_diff::apply_text_diff;
use crate::sync::values;

const CONTENT: &str = "content";
const META: &str = "meta";
const EDITS: &str = "edits";
const TAGS: &str = "tags";
const FOLDER: &str = "folder";
const SORT_ORDER: &str = "sort_order";
const COLOR: &str = "color";
const CREATED_AT: &str = "created_at";
const UPDATED_AT: &str = "updated_at";

/// Document CRDT d'une note, ouvert pour l'appareil courant.
pub struct NoteDoc {
    doc: Doc,
    content: TextRef,
    meta: MapRef,
    edits: MapRef,
}

/// Une note locale, telle que l'adaptateur la lit (uuid à part : c'est l'identifiant du
/// document).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteFields {
    /// Texte markdown complet.
    pub content: String,
    /// Tags, dans leur ordre.
    pub tags: Vec<String>,
    /// Dossier, `None` à la racine.
    pub folder: Option<Uuid>,
    /// Position dans le dossier.
    pub sort_order: Option<i64>,
    /// Couleur `#rrggbb`.
    pub color: Option<String>,
    /// Date de création (format SQLite).
    pub created_at: String,
    /// Date de dernière modification (format SQLite).
    pub updated_at: String,
}

/// Modification du contenu ou des tags ; `None` laisse le champ tel quel.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct NoteEdit<'a> {
    /// Nouveau contenu complet (celui de l'éditeur).
    pub content: Option<&'a str>,
    /// Nouveaux tags.
    pub tags: Option<&'a [String]>,
}

/// La note à écrire dans SQLite : même état CRDT, même projection, sur tous les appareils.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedNote {
    /// Identifiant du document.
    pub uuid: Uuid,
    /// Titre, dérivé de la première ligne du contenu.
    pub title: String,
    /// Texte markdown complet.
    pub content: String,
    /// Tags normalisés.
    pub tags: Vec<String>,
    /// Dossier voulu, gardé même s'il est inconnu ou supprimé (la note est alors rangée à la
    /// racine, voir [`crate::sync::space_doc::ProjectedSpace::resolve_folder`]).
    pub folder_uuid: Option<Uuid>,
    /// Position dans le dossier.
    pub sort_order: Option<i64>,
    /// Couleur `#rrggbb`, `None` sans couleur ou si la valeur reçue est invalide.
    pub color: Option<String>,
    /// Date de création.
    pub created_at: String,
    /// Date de dernière modification.
    pub updated_at: String,
    /// Date de suppression, `None` si la note est visible (jamais supprimée, ou restaurée par
    /// une modification concurrente de la suppression).
    pub deleted_at: Option<String>,
}

impl ProjectedNote {
    /// La note est supprimée depuis au moins 30 jours à la date `now` : son contenu peut être
    /// vidé (le blob jeté), seule la tombstone reste.
    pub fn is_purgeable(&self, now: &str) -> bool {
        self.deleted_at
            .as_deref()
            .is_some_and(|deleted_at| is_purgeable(deleted_at, now))
    }
}

impl sealed::Inner for NoteDoc {
    fn inner(&self) -> &Doc {
        &self.doc
    }
}

impl SyncDoc for NoteDoc {}

impl NoteDoc {
    fn open(doc: Doc) -> Self {
        let content = doc.get_or_insert_text(CONTENT);
        let meta = doc.get_or_insert_map(META);
        let edits = doc.get_or_insert_map(EDITS);
        Self {
            doc,
            content,
            meta,
            edits,
        }
    }

    /// Note sans aucune opération, pour y appliquer un document reçu.
    pub fn empty(device: &DeviceKey) -> Self {
        Self::open(new_doc(device))
    }

    /// Le document stocké (blob), ouvert pour l'appareil.
    pub fn load(device: &DeviceKey, blob: &Update) -> SyncResult<Self> {
        let note = Self::empty(device);
        note.apply_update(blob)?;
        Ok(note)
    }

    /// Marque une modification du contenu par cet appareil : l'horloge courante de
    /// l'appareil, prise avant l'écriture de la marque elle-même.
    fn mark_edit(&self, transaction: &mut TransactionMut) {
        let client = self.doc.client_id();
        let clock = transaction.state_vector().get(&client);
        let value = values::optional_integer(Some(i64::from(clock)));

        values::set(&self.edits, transaction, &client.get().to_string(), value);
    }
}

/// Document d'une note locale qui n'en a pas encore (note créée, ou pas encore migrée).
///
/// À n'appeler qu'une fois par uuid et par appareil, puis à stocker : reconstruire le
/// document réutiliserait les mêmes horloges pour un autre contenu.
pub fn from_note(device: &DeviceKey, note: &NoteFields) -> NoteDoc {
    let doc = NoteDoc::empty(device);
    {
        let mut transaction = doc.doc.transact_mut();
        doc.content.insert(&mut transaction, 0, &note.content);
        doc.mark_edit(&mut transaction);
        write_fields(&doc.meta, &mut transaction, note);
    }
    doc
}

fn write_fields(meta: &MapRef, transaction: &mut TransactionMut, note: &NoteFields) {
    values::set(meta, transaction, TAGS, values::text_list(&note.tags));
    values::set(
        meta,
        transaction,
        FOLDER,
        values::optional_uuid(note.folder),
    );
    values::set(
        meta,
        transaction,
        SORT_ORDER,
        values::optional_integer(note.sort_order),
    );
    values::set(
        meta,
        transaction,
        COLOR,
        values::optional_text(note.color.as_deref()),
    );
    values::set(
        meta,
        transaction,
        CREATED_AT,
        note.created_at.as_str().into(),
    );
    values::set(
        meta,
        transaction,
        UPDATED_AT,
        note.updated_at.as_str().into(),
    );
}

/// Modifie le contenu (par diff minimal) et les tags ; `updated_at` passe à `now` si quelque
/// chose a changé. Rend `true` si le document a changé.
pub fn apply_changes(doc: &NoteDoc, edit: &NoteEdit<'_>, now: &str) -> bool {
    let mut transaction = doc.doc.transact_mut();
    let mut changed = false;

    if let Some(content) = edit.content {
        let current = doc.content.get_string(&transaction);
        if current != content {
            apply_text_diff(&doc.content, &mut transaction, &current, content);
            doc.mark_edit(&mut transaction);
            changed = true;
        }
    }
    if let Some(tags) = edit.tags {
        changed |= values::set(&doc.meta, &mut transaction, TAGS, values::text_list(tags));
    }
    if changed {
        values::set(&doc.meta, &mut transaction, UPDATED_AT, now.into());
    }
    changed
}

/// Range la note (dossier et position déjà calculés par l'appelant, avec les règles du
/// stockage) ; `updated_at` n'est pas touché. Rend `true` si le document a changé.
pub fn apply_placement(doc: &NoteDoc, folder: Option<Uuid>, sort_order: Option<i64>) -> bool {
    let mut transaction = doc.doc.transact_mut();
    let moved = values::set(
        &doc.meta,
        &mut transaction,
        FOLDER,
        values::optional_uuid(folder),
    );
    let order = values::optional_integer(sort_order);

    values::set(&doc.meta, &mut transaction, SORT_ORDER, order) | moved
}

/// Colore la note ; `updated_at` n'est pas touché. Rend `true` si le document a changé.
pub fn apply_color(doc: &NoteDoc, color: Option<&str>) -> bool {
    let mut transaction = doc.doc.transact_mut();
    values::set(
        &doc.meta,
        &mut transaction,
        COLOR,
        values::optional_text(color),
    )
}

/// Supprime la note à la date `now` (tombstone). Une modification du contenu que cet
/// appareil n'a pas encore vue la restaurera. Rend `true` si la note était visible.
pub fn apply_delete(doc: &NoteDoc, now: &str) -> bool {
    let mut transaction = doc.doc.transact_mut();
    if deletion::deleted_at(doc, &transaction).is_some() {
        return false;
    }
    deletion::write(doc, &mut transaction, now);
    true
}

/// Restaure une note supprimée. Rend `true` si elle était supprimée.
pub fn apply_restore(doc: &NoteDoc) -> bool {
    let mut transaction = doc.doc.transact_mut();
    if deletion::deleted_at(doc, &transaction).is_none() {
        return false;
    }
    deletion::clear(doc, &mut transaction);
    true
}

/// La ligne SQLite de la note : valeurs reçues validées, dates mal formées remplacées par
/// [`FALLBACK_TIMESTAMP`](crate::sync::FALLBACK_TIMESTAMP), suppression arbitrée contre les
/// modifications concurrentes.
pub fn project(doc: &NoteDoc, uuid: Uuid) -> ProjectedNote {
    let transaction = doc.doc.transact();
    let meta = &doc.meta;
    let content = doc.content.get_string(&transaction);

    ProjectedNote {
        uuid,
        title: extract_title(&content),
        tags: normalize_tags(&values::texts(meta, &transaction, TAGS)),
        folder_uuid: values::uuid(meta, &transaction, FOLDER),
        sort_order: values::integer(meta, &transaction, SORT_ORDER),
        color: normalize_color(values::text(meta, &transaction, COLOR).as_deref()),
        created_at: values::timestamp(meta, &transaction, CREATED_AT),
        updated_at: values::timestamp(meta, &transaction, UPDATED_AT),
        deleted_at: deletion::deleted_at(doc, &transaction),
        content,
    }
}
