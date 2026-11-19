//! Lecture et écriture des valeurs des `Y.Map`.
//!
//! Les valeurs lues viennent d'autres appareils : un type inattendu vaut une valeur absente,
//! jamais une erreur ni une panique.

use std::sync::Arc;

use domain::sync::is_timestamp;
use domain::uuid::Uuid;
use yrs::{Any, Map, MapRef, Number, Out, ReadTxn, TransactionMut};

/// Date de repli d'une date absente ou mal formée reçue d'un autre appareil.
pub const FALLBACK_TIMESTAMP: &str = "1970-01-01 00:00:00";

/// Valeur brute d'une clé, si c'est une valeur simple.
fn any<T: ReadTxn>(map: &MapRef, transaction: &T, key: &str) -> Option<Any> {
    match map.get(transaction, key)? {
        Out::Any(value) => Some(value),
        _ => None,
    }
}

/// Texte d'une clé.
pub(crate) fn text<T: ReadTxn>(map: &MapRef, transaction: &T, key: &str) -> Option<String> {
    match any(map, transaction, key)? {
        Any::String(value) => Some(value.to_string()),
        _ => None,
    }
}

/// Entier d'une clé ; un nombre flottant entier (écrit par JavaScript) est accepté.
pub(crate) fn integer<T: ReadTxn>(map: &MapRef, transaction: &T, key: &str) -> Option<i64> {
    match any(map, transaction, key)? {
        Any::Number(Number::Int(value)) => Some(value),
        Any::Number(Number::Float(value)) => float_to_integer(value),
        _ => None,
    }
}

/// Octets d'une clé.
pub(crate) fn bytes<T: ReadTxn>(map: &MapRef, transaction: &T, key: &str) -> Option<Vec<u8>> {
    match any(map, transaction, key)? {
        Any::Buffer(value) => Some(value.to_vec()),
        _ => None,
    }
}

/// Date (format SQLite) d'une clé, [`FALLBACK_TIMESTAMP`] si elle est absente ou mal formée.
pub(crate) fn timestamp<T: ReadTxn>(map: &MapRef, transaction: &T, key: &str) -> String {
    optional_timestamp(map, transaction, key).unwrap_or_else(|| FALLBACK_TIMESTAMP.to_owned())
}

/// Date facultative d'une clé ; une date présente mais mal formée vaut [`FALLBACK_TIMESTAMP`].
pub(crate) fn optional_timestamp<T: ReadTxn>(
    map: &MapRef,
    transaction: &T,
    key: &str,
) -> Option<String> {
    let value = text(map, transaction, key)?;
    Some(if is_timestamp(&value) {
        value
    } else {
        FALLBACK_TIMESTAMP.to_owned()
    })
}

/// Uuid écrit sous forme canonique d'une clé.
pub(crate) fn uuid<T: ReadTxn>(map: &MapRef, transaction: &T, key: &str) -> Option<Uuid> {
    text(map, transaction, key).and_then(|value| Uuid::parse(&value))
}

/// Liste de textes d'une clé ; les éléments d'un autre type sont ignorés.
pub(crate) fn texts<T: ReadTxn>(map: &MapRef, transaction: &T, key: &str) -> Vec<String> {
    let Some(Any::Array(items)) = any(map, transaction, key) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| match item {
            Any::String(value) => Some(value.to_string()),
            _ => None,
        })
        .collect()
}

/// Écrit `value` sous `key` seulement si elle diffère (une clé absente vaut `null`) : une
/// écriture identique créerait une opération inutile, échangée et stockée. Rend `true` si
/// la valeur a changé.
pub(crate) fn set(map: &MapRef, transaction: &mut TransactionMut, key: &str, value: Any) -> bool {
    if any(map, transaction, key).unwrap_or(Any::Null) == value {
        return false;
    }
    map.insert(transaction, key, value);
    true
}

/// Texte facultatif (`null` quand absent).
pub(crate) fn optional_text(value: Option<&str>) -> Any {
    value.map_or(Any::Null, Any::from)
}

/// Entier facultatif (`null` quand absent).
pub(crate) fn optional_integer(value: Option<i64>) -> Any {
    value.map_or(Any::Null, |value| Any::Number(Number::Int(value)))
}

/// Uuid facultatif, sous forme canonique (`null` quand absent).
pub(crate) fn optional_uuid(value: Option<Uuid>) -> Any {
    value.map_or(Any::Null, |uuid| Any::from(uuid.to_string()))
}

/// Liste de textes.
pub(crate) fn text_list(values: &[String]) -> Any {
    let items: Vec<Any> = values
        .iter()
        .map(|value| Any::from(value.as_str()))
        .collect();
    Any::Array(Arc::from(items))
}

/// Seuls les entiers représentables exactement en JavaScript sont acceptés.
fn float_to_integer(value: f64) -> Option<i64> {
    let safe = value.fract() == 0.0 && value.abs() <= Number::F64_MAX_SAFE_INTEGER;
    // Exact : la valeur est entière et bornée à 2⁵³
    safe.then_some(value as i64)
}

#[cfg(test)]
mod tests {
    use domain::sync::DeviceKey;
    use yrs::Transact;

    use super::*;
    use crate::sync::client::new_doc;

    #[test]
    fn reads_values_written_in_any_form() {
        let doc = new_doc(&DeviceKey::from_bytes([1; 32]));
        let map = doc.get_or_insert_map("meta");
        let mut transaction = doc.transact_mut();

        map.insert(&mut transaction, "float", 3.0_f64);
        map.insert(&mut transaction, "fraction", 3.5_f64);
        map.insert(
            &mut transaction,
            "list",
            text_list(&["a".into(), "b".into()]),
        );
        map.insert(&mut transaction, "uuid", "not a uuid");

        assert_eq!(integer(&map, &transaction, "float"), Some(3));
        assert_eq!(integer(&map, &transaction, "fraction"), None);
        assert_eq!(texts(&map, &transaction, "list"), ["a", "b"]);
        assert_eq!(text(&map, &transaction, "float"), None);
        assert_eq!(uuid(&map, &transaction, "uuid"), None);
        assert_eq!(bytes(&map, &transaction, "missing"), None);
    }

    #[test]
    fn skips_writes_that_change_nothing() {
        let doc = new_doc(&DeviceKey::from_bytes([1; 32]));
        let map = doc.get_or_insert_map("meta");
        let mut transaction = doc.transact_mut();

        assert!(!set(&map, &mut transaction, "color", Any::Null));
        assert!(set(
            &map,
            &mut transaction,
            "color",
            optional_text(Some("#aabbcc"))
        ));
        assert!(!set(
            &map,
            &mut transaction,
            "color",
            optional_text(Some("#aabbcc"))
        ));
        assert!(set(&map, &mut transaction, "color", optional_text(None)));
        assert!(!set(&map, &mut transaction, "color", Any::Null));
    }
}
