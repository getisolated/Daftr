//! Encodage des tags dans la colonne `notes.tags` (tableau JSON de chaînes).

/// Tableau JSON des tags.
pub fn encode(tags: &[String]) -> String {
    serde_json::Value::from(tags.to_vec()).to_string()
}

/// Les tags d'une colonne ; les éléments non textuels et une colonne malformée sont ignorés.
pub fn decode(column: &str) -> Vec<String> {
    let Ok(serde_json::Value::Array(items)) = serde_json::from_str(column) else {
        return Vec::new();
    };
    items
        .into_iter()
        .filter_map(|item| item.as_str().map(str::to_owned))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_tags() {
        let tags = vec!["a\"b".to_owned(), "été".to_owned()];
        assert_eq!(decode(&encode(&tags)), tags);
        assert_eq!(encode(&[]), "[]");
    }

    #[test]
    fn ignores_malformed_columns_and_items() {
        assert!(decode("not json").is_empty());
        assert!(decode("{\"a\":1}").is_empty());
        assert_eq!(decode("[\"x\", 3, null]"), ["x"]);
    }
}
