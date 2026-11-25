//! Suppression d'une note contre modification concurrente.
//!
//! La suppression écrit `deleted_at` et `deleted_sv`, le state vector au moment de
//! supprimer. La note n'est supprimée que si aucune modification du contenu n'est inconnue
//! de ce state vector : chaque appareil note dans `edits` l'horloge de sa dernière
//! modification, avant d'écrire la marque ; elle est inconnue de la suppression si cette
//! horloge atteint celle du state vector pour cet appareil. Une simple suppression de
//! caractères est ainsi détectée, alors qu'elle n'avance pas le state vector elle-même.

use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{Any, ClientID, Map, ReadTxn, StateVector, TransactionMut};

use super::NoteDoc;
use crate::sync::values;

const DELETED_AT: &str = "deleted_at";
const DELETED_SV: &str = "deleted_sv";

/// Un identifiant client Yjs tient sur 53 bits.
const MAX_CLIENT_ID: u64 = (1 << 53) - 1;

/// Marque la note supprimée à la date `now`.
pub(super) fn write(doc: &NoteDoc, transaction: &mut TransactionMut, now: &str) {
    let seen = transaction.state_vector().encode_v1();

    values::set(&doc.meta, transaction, DELETED_SV, Any::from(seen));
    values::set(&doc.meta, transaction, DELETED_AT, now.into());
}

/// Retire la marque de suppression.
pub(super) fn clear(doc: &NoteDoc, transaction: &mut TransactionMut) {
    values::set(&doc.meta, transaction, DELETED_AT, Any::Null);
    values::set(&doc.meta, transaction, DELETED_SV, Any::Null);
}

/// Date de suppression si la note est supprimée : marque présente et valide, et aucune
/// modification du contenu ignorée par la suppression. Dans le doute (state vector illisible),
/// la note reste visible : perdre une note est pire que la garder.
pub(super) fn deleted_at<T: ReadTxn>(doc: &NoteDoc, transaction: &T) -> Option<String> {
    let deleted_at = values::optional_timestamp(&doc.meta, transaction, DELETED_AT)?;
    let seen = values::bytes(&doc.meta, transaction, DELETED_SV)?;
    let seen = StateVector::decode_v1(&seen).ok()?;

    (!edited_since(doc, transaction, &seen)).then_some(deleted_at)
}

/// Une modification du contenu est inconnue du state vector `seen`.
fn edited_since<T: ReadTxn>(doc: &NoteDoc, transaction: &T, seen: &StateVector) -> bool {
    doc.edits.iter(transaction).any(|(key, _)| {
        let Some(client) = parse_client(key) else {
            return false;
        };
        let clock = values::integer(&doc.edits, transaction, key);
        clock.is_some_and(|clock| clock >= i64::from(seen.get(&client)))
    })
}

fn parse_client(text: &str) -> Option<ClientID> {
    let value: u64 = text.parse().ok()?;
    (value <= MAX_CLIENT_ID).then(|| ClientID::new(value))
}
