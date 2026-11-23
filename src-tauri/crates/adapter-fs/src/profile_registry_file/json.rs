//! Forme JSON du registre des profils :
//! `{ "version": 1, "lastActive": "default", "profiles": [{ "id", "name", "icon",
//! "passwordHash" }] }`, les champs absents valant `null`.

use domain::profile::{IconFormat, Profile, ProfileId, ProfileRegistry, normalize_profile_name};
use serde_json::{Map, Value, json};

/// Version du format, pour une évolution future.
const FORMAT_VERSION: u64 = 1;

/// Le registre en JSON.
pub(super) fn registry_to_json(registry: &ProfileRegistry) -> Value {
    let profiles: Vec<Value> = registry.profiles().iter().map(profile_to_json).collect();
    json!({
        "version": FORMAT_VERSION,
        "lastActive": registry.last_active().as_str(),
        "profiles": profiles,
    })
}

fn profile_to_json(profile: &Profile) -> Value {
    json!({
        "id": profile.id.as_str(),
        "name": profile.name,
        "icon": profile.icon.map(IconFormat::as_str),
        "passwordHash": profile.password_hash,
    })
}

/// Le registre lu, `None` si le JSON n'a pas la forme attendue. Une entrée illisible est
/// ignorée (journalisée) ; le registre est ensuite rendu cohérent (profil par défaut…).
pub(super) fn registry_from_json(value: &Value) -> Option<ProfileRegistry> {
    let object = value.as_object()?;
    let entries = object.get("profiles")?.as_array()?;
    let profiles: Vec<Profile> = entries.iter().filter_map(profile_from_json).collect();
    let last_active = text(object, "lastActive").and_then(ProfileId::parse);

    Some(ProfileRegistry::new(profiles, last_active))
}

fn profile_from_json(value: &Value) -> Option<Profile> {
    let object = value.as_object()?;
    let Some(id) = text(object, "id").and_then(ProfileId::parse) else {
        tracing::warn!("profil sans identifiant valide ignoré");
        return None;
    };
    Some(Profile {
        name: normalize_profile_name(text(object, "name")),
        icon: text(object, "icon").and_then(IconFormat::parse),
        password_hash: text(object, "passwordHash")
            .filter(|hash| !hash.is_empty())
            .map(str::to_owned),
        id,
    })
}

fn text<'a>(object: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    object.get(key).and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_null_for_what_a_profile_does_not_have() {
        let value = registry_to_json(&ProfileRegistry::default());

        assert_eq!(
            value,
            json!({
                "version": 1,
                "lastActive": "default",
                "profiles": [{ "id": "default", "name": null, "icon": null, "passwordHash": null }],
            })
        );
    }

    #[test]
    fn skips_invalid_entries_and_fields() {
        let value = json!({
            "lastActive": "work",
            "profiles": [
                { "id": "../evil", "name": "x" },
                "not an object",
                { "id": "work", "name": "  Job  ", "icon": "../../x.exe", "passwordHash": 3 },
            ],
        });

        let registry = registry_from_json(&value).unwrap();

        let work = registry.get(&ProfileId::parse("work").unwrap()).unwrap();
        assert_eq!(work.name.as_deref(), Some("Job"));
        assert_eq!(work.icon, None);
        assert_eq!(work.password_hash, None);
        assert_eq!(
            registry.profiles().len(),
            2,
            "le profil par défaut est ajouté"
        );
        assert_eq!(registry.last_active().as_str(), "work");
    }

    #[test]
    fn refuses_a_document_without_profiles() {
        assert_eq!(
            registry_from_json(&json!({ "lastActive": "default" })),
            None
        );
        assert_eq!(registry_from_json(&json!("text")), None);
    }
}
