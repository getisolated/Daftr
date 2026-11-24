//! Nom d'un appareil et code d'appairage.

use std::fmt;

use crate::text::truncate_chars;

/// Longueur maximale d'un nom d'appareil, en caractères.
pub const MAX_DEVICE_NAME_LENGTH: usize = 64;

/// Nombre de chiffres d'un code d'appairage.
pub const PAIRING_CODE_DIGITS: usize = 6;

/// Nom affiché d'un appareil (le nom d'hôte par défaut) : sans espaces autour ni caractère de
/// contrôle, de 1 à [`MAX_DEVICE_NAME_LENGTH`] caractères.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DeviceName(String);

impl DeviceName {
    /// Le nom sans les espaces autour, `None` s'il est vide, trop long ou contient un
    /// caractère de contrôle.
    ///
    /// ```
    /// use domain::sync::DeviceName;
    ///
    /// assert_eq!(DeviceName::parse("  Laptop ").unwrap().as_str(), "Laptop");
    /// assert!(DeviceName::parse("   ").is_none());
    /// ```
    pub fn parse(name: &str) -> Option<Self> {
        let trimmed = name.trim();
        let length = trimmed.chars().count();
        let valid = (1..=MAX_DEVICE_NAME_LENGTH).contains(&length)
            && !trimmed.chars().any(char::is_control);

        valid.then(|| Self(trimmed.to_owned()))
    }

    /// Un nom reçu d'un autre appareil (donnée non fiable) rendu valide : caractères de
    /// contrôle retirés, coupé à la longueur maximale ; `None` s'il ne reste rien.
    ///
    /// ```
    /// use domain::sync::DeviceName;
    ///
    /// assert_eq!(DeviceName::sanitize("PC\u{0}\n").unwrap().as_str(), "PC");
    /// ```
    pub fn sanitize(name: &str) -> Option<Self> {
        let visible: String = name.chars().filter(|c| !c.is_control()).collect();
        Self::parse(truncate_chars(visible.trim(), MAX_DEVICE_NAME_LENGTH))
    }

    /// Le nom.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DeviceName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Code d'appairage : exactement [`PAIRING_CODE_DIGITS`] chiffres ASCII (zéros de tête
/// compris). Il n'est jamais annoncé sur le réseau.
#[derive(Clone, PartialEq, Eq)]
pub struct PairingCode(String);

impl PairingCode {
    /// Le code saisi, espaces autour et au milieu ignorés (`123 456`), `None` s'il n'a pas
    /// exactement six chiffres.
    ///
    /// ```
    /// use domain::sync::PairingCode;
    ///
    /// assert_eq!(PairingCode::parse(" 012 345 ").unwrap().as_str(), "012345");
    /// assert!(PairingCode::parse("12345").is_none());
    /// ```
    pub fn parse(code: &str) -> Option<Self> {
        let digits: String = code.chars().filter(|c| !c.is_whitespace()).collect();
        let valid =
            digits.len() == PAIRING_CODE_DIGITS && digits.bytes().all(|b| b.is_ascii_digit());

        valid.then_some(Self(digits))
    }

    /// Le code d'un nombre tiré au hasard par l'appelant, `None` s'il dépasse six chiffres.
    ///
    /// ```
    /// use domain::sync::PairingCode;
    ///
    /// assert_eq!(PairingCode::from_number(42).unwrap().as_str(), "000042");
    /// assert!(PairingCode::from_number(1_000_000).is_none());
    /// ```
    pub fn from_number(number: u32) -> Option<Self> {
        (number < 1_000_000).then(|| Self(format!("{number:06}")))
    }

    /// Les six chiffres.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Le code n'apparaît jamais dans les traces.
impl fmt::Debug for PairingCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("PairingCode(******)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_names_have_a_bounded_length() {
        let at_limit = "é".repeat(MAX_DEVICE_NAME_LENGTH);
        assert!(DeviceName::parse(&at_limit).is_some());
        assert!(DeviceName::parse(&format!("{at_limit}x")).is_none());
        assert!(DeviceName::parse("Bureau\tPC").is_none());
    }

    #[test]
    fn sanitized_names_are_cut_and_cleaned() {
        let long = "a".repeat(MAX_DEVICE_NAME_LENGTH + 10);
        let name = DeviceName::sanitize(&long).unwrap();
        assert_eq!(name.as_str().len(), MAX_DEVICE_NAME_LENGTH);
        assert!(DeviceName::sanitize("\u{7}\n").is_none());
    }

    #[test]
    fn pairing_codes_are_six_ascii_digits() {
        assert!(PairingCode::parse("１２３４５６").is_none());
        assert!(PairingCode::parse("1234567").is_none());
        assert!(PairingCode::parse("12a456").is_none());
        assert_eq!(PairingCode::parse("999999").unwrap().as_str(), "999999");
    }

    #[test]
    fn pairing_codes_stay_out_of_debug_output() {
        let code = PairingCode::parse("123456").unwrap();
        assert!(!format!("{code:?}").contains("123456"));
    }
}
