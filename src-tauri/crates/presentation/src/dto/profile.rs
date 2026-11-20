//! Profils.

use domain::profile::Profile;
use serde::Serialize;
use ts_rs::TS;

/// Un profil, sans son empreinte de mot de passe (qui ne quitte jamais le processus principal).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProfileDto {
    /// Identifiant.
    pub id: String,
    /// Nom, `null` sans nom (le renderer affiche un nom par défaut traduit).
    pub name: Option<String>,
    /// Une icône est enregistrée (`profiles_icon` la rend).
    pub has_icon: bool,
    /// Protégé par un mot de passe maître.
    pub protected: bool,
    /// Le profil par défaut (jamais supprimable).
    pub is_default: bool,
}

impl From<&Profile> for ProfileDto {
    fn from(profile: &Profile) -> Self {
        Self {
            id: profile.id.as_str().to_owned(),
            name: profile.name.clone(),
            has_icon: profile.icon.is_some(),
            protected: profile.is_protected(),
            is_default: profile.id.is_default(),
        }
    }
}

/// Les profils et celui qui est actif.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProfilesStateDto {
    /// Tous les profils, le profil par défaut d'abord.
    pub profiles: Vec<ProfileDto>,
    /// Profil actif.
    pub active_id: String,
    /// Le profil actif attend son mot de passe : ses données sont fermées.
    pub locked: bool,
}

/// Ce qui a quitté le profil actif lors d'un transfert.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TransferResultDto {
    /// Notes, celles des dossiers comprises.
    #[ts(type = "number[]")]
    pub note_ids: Vec<i64>,
    /// Dossiers, sous-dossiers compris.
    #[ts(type = "number[]")]
    pub folder_ids: Vec<i64>,
    /// Collections, par chemin.
    pub collections: Vec<String>,
}

#[cfg(test)]
mod tests {
    use domain::profile::{IconFormat, ProfileId};

    use super::*;

    #[test]
    fn never_exposes_the_password_hash() {
        let profile = Profile {
            name: Some("Work".to_owned()),
            icon: Some(IconFormat::Png),
            password_hash: Some("$argon2id$secret".to_owned()),
            ..Profile::new(ProfileId::parse("work").unwrap())
        };

        let json = serde_json::to_value(ProfileDto::from(&profile)).unwrap();

        assert_eq!(
            json,
            serde_json::json!({
                "id": "work", "name": "Work", "hasIcon": true, "protected": true, "isDefault": false,
            })
        );
    }
}
