//! Profils : chacun a sa propre base de notes (notes, dossiers, collections) ; les réglages
//! restent communs à tous.
//!
//! Le profil par défaut garde les données d'avant les profils, à la racine du dossier de
//! données ; les autres vivent chacun dans un sous-dossier nommé d'après leur identifiant.

mod icon;
mod registry;

pub use icon::{IconFormat, MAX_ICON_BYTES, validate_icon};
pub use registry::ProfileRegistry;

/// Identifiant du profil par défaut.
pub const DEFAULT_PROFILE_ID: &str = "default";
/// Longueur maximale d'un identifiant.
pub const MAX_PROFILE_ID_LENGTH: usize = 32;
/// Longueur maximale d'un nom de profil, en caractères.
pub const MAX_PROFILE_NAME_LENGTH: usize = 64;
/// Longueur maximale d'un mot de passe maître, en octets : borne le coût du hachage.
pub const MAX_PASSWORD_BYTES: usize = 1024;

/// Identifiant d'un profil : minuscules ASCII, chiffres et tirets. Il sert de nom de
/// dossier : rien d'autre n'y est admis (ni séparateur, ni `..`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProfileId(String);

impl ProfileId {
    /// L'identifiant s'il est valide.
    ///
    /// ```
    /// use domain::profile::ProfileId;
    /// assert!(ProfileId::parse("a1b2-c3").is_some());
    /// assert!(ProfileId::parse("../x").is_none());
    /// ```
    pub fn parse(text: &str) -> Option<Self> {
        let valid_char = |c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-';
        let valid =
            !text.is_empty() && text.len() <= MAX_PROFILE_ID_LENGTH && text.chars().all(valid_char);

        valid.then(|| Self(text.to_owned()))
    }

    /// L'identifiant du profil par défaut.
    pub fn default_profile() -> Self {
        Self(DEFAULT_PROFILE_ID.to_owned())
    }

    /// C'est le profil par défaut.
    pub fn is_default(&self) -> bool {
        self.0 == DEFAULT_PROFILE_ID
    }

    /// Forme textuelle.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Un profil enregistré.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    /// Identifiant (et nom de son dossier).
    pub id: ProfileId,
    /// Nom choisi, `None` sans nom (le renderer affiche un nom par défaut traduit).
    pub name: Option<String>,
    /// Format de l'icône enregistrée dans le dossier du profil, `None` sans icône.
    pub icon: Option<IconFormat>,
    /// Empreinte du mot de passe maître (chaîne PHC), `None` sans mot de passe.
    pub password_hash: Option<String>,
}

impl Profile {
    /// Profil sans nom, sans icône ni mot de passe.
    pub fn new(id: ProfileId) -> Self {
        Self {
            id,
            name: None,
            icon: None,
            password_hash: None,
        }
    }

    /// Protégé par un mot de passe maître.
    pub fn is_protected(&self) -> bool {
        self.password_hash.is_some()
    }
}

/// Le nom sans les espaces autour, tronqué à [`MAX_PROFILE_NAME_LENGTH`] caractères ; `None`
/// pour un nom vide (le profil n'a alors pas de nom).
///
/// ```
/// use domain::profile::normalize_profile_name;
/// assert_eq!(normalize_profile_name(Some("  Travail ")), Some("Travail".to_owned()));
/// assert_eq!(normalize_profile_name(Some("   ")), None);
/// ```
pub fn normalize_profile_name(name: Option<&str>) -> Option<String> {
    let trimmed = name?.trim();
    let truncated: String = trimmed.chars().take(MAX_PROFILE_NAME_LENGTH).collect();
    let kept = truncated.trim_end();

    (!kept.is_empty()).then(|| kept.to_owned())
}

/// Un mot de passe acceptable : non vide et pas trop long.
pub fn is_acceptable_password(password: &str) -> bool {
    !password.is_empty() && password.len() <= MAX_PASSWORD_BYTES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_folder_safe_identifiers() {
        for valid in [
            "default",
            "a",
            "0f3e-9b",
            &"a".repeat(MAX_PROFILE_ID_LENGTH),
        ] {
            assert!(ProfileId::parse(valid).is_some(), "{valid}");
        }
        let too_long = "a".repeat(MAX_PROFILE_ID_LENGTH + 1);
        for invalid in [
            "",
            "..",
            "a/b",
            "a\\b",
            "A",
            "é",
            "a b",
            "a.b",
            too_long.as_str(),
        ] {
            assert!(ProfileId::parse(invalid).is_none(), "{invalid}");
        }
    }

    #[test]
    fn knows_the_default_profile() {
        assert!(ProfileId::default_profile().is_default());
        assert!(!ProfileId::parse("work").unwrap().is_default());
    }

    #[test]
    fn trims_and_truncates_the_name() {
        assert_eq!(
            normalize_profile_name(Some(" Perso ")).as_deref(),
            Some("Perso")
        );
        assert_eq!(normalize_profile_name(Some("")), None);
        assert_eq!(normalize_profile_name(None), None);

        let long = "é".repeat(MAX_PROFILE_NAME_LENGTH + 10);
        let kept = normalize_profile_name(Some(&long)).unwrap();
        assert_eq!(kept.chars().count(), MAX_PROFILE_NAME_LENGTH);
    }

    #[test]
    fn a_profile_is_protected_only_with_a_hash() {
        let mut profile = Profile::new(ProfileId::default_profile());
        assert!(!profile.is_protected());

        profile.password_hash = Some("$argon2id$…".to_owned());
        assert!(profile.is_protected());
    }

    #[test]
    fn bounds_the_password() {
        assert!(is_acceptable_password("x"));
        assert!(!is_acceptable_password(""));
        assert!(!is_acceptable_password(&"x".repeat(MAX_PASSWORD_BYTES + 1)));
    }
}
