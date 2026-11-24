//! UUID des notes et des dossiers : identifiants stables, échangés entre appareils.
//!
//! Le domaine ne tire aucune dépendance : l'aléa d'un UUID version 4 est fourni par l'appelant,
//! et la version 5 (dérivée) s'appuie sur un SHA-1 interne.

mod sha1;

use std::fmt;

use crate::hex;

/// Un UUID (RFC 9562). Sa forme textuelle est l'écriture canonique en minuscules ; l'ordre
/// des valeurs est celui de cette écriture (utile pour départager de façon déterministe).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Uuid([u8; 16]);

impl Uuid {
    /// Un UUID version 4 tiré de 16 octets aléatoires (fournis par l'appelant) : les bits de
    /// version et de variante sont imposés.
    pub fn from_random(bytes: [u8; 16]) -> Self {
        Self::with_version(bytes, 4)
    }

    /// Un UUID version 5 : SHA-1 de `namespace` suivi de `name`, tronqué à 16 octets. Avec un
    /// espace de noms de 16 octets, c'est exactement l'UUIDv5 de la RFC 9562.
    ///
    /// ```
    /// use domain::uuid::Uuid;
    ///
    /// let dns = Uuid::parse("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap();
    /// let derived = Uuid::derived(dns.as_bytes(), "www.example.com");
    /// assert_eq!(derived.to_string(), "2ed6657d-e927-568b-95e1-2665a8aea6a2");
    /// ```
    pub fn derived(namespace: &[u8], name: &str) -> Self {
        let digest = sha1::digest(&[namespace, name.as_bytes()]);
        let mut bytes = [0_u8; 16];
        bytes.copy_from_slice(&digest[..16]);

        Self::with_version(bytes, 5)
    }

    /// Les octets tels quels (aucun bit imposé).
    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    /// Les 16 octets.
    pub const fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// L'UUID écrit sous sa forme canonique (`8-4-4-4-12` chiffres hexadécimaux, majuscules
    /// acceptées), `None` pour tout autre texte.
    ///
    /// ```
    /// use domain::uuid::Uuid;
    ///
    /// let uuid = Uuid::parse("6BA7B810-9DAD-11D1-80B4-00C04FD430C8").unwrap();
    /// assert_eq!(uuid.to_string(), "6ba7b810-9dad-11d1-80b4-00c04fd430c8");
    /// assert!(Uuid::parse("6ba7b8109dad11d180b400c04fd430c8").is_none());
    /// ```
    pub fn parse(text: &str) -> Option<Self> {
        let groups: Vec<&str> = text.split('-').collect();
        let lengths: Vec<usize> = groups.iter().map(|group| group.len()).collect();
        if lengths != [8, 4, 4, 4, 12] {
            return None;
        }
        hex::decode::<16>(&groups.concat()).map(Self)
    }

    fn with_version(mut bytes: [u8; 16], version: u8) -> Self {
        bytes[6] = (bytes[6] & 0x0F) | (version << 4);
        bytes[8] = (bytes[8] & 0x3F) | 0x80;
        Self(bytes)
    }
}

impl fmt::Display for Uuid {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = hex::encode(&self.0);
        let groups = [
            &text[..8],
            &text[8..12],
            &text[12..16],
            &text[16..20],
            &text[20..],
        ];
        formatter.write_str(&groups.join("-"))
    }
}

impl fmt::Debug for Uuid {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Uuid({self})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_uuids_carry_version_4() {
        let uuid = Uuid::from_random([0xFF; 16]);
        assert_eq!(uuid.to_string(), "ffffffff-ffff-4fff-bfff-ffffffffffff");
    }

    #[test]
    fn derived_uuids_are_stable_and_depend_on_both_inputs() {
        let first = Uuid::derived(b"space", "default-folder");
        assert_eq!(first, Uuid::derived(b"space", "default-folder"));
        assert_ne!(first, Uuid::derived(b"other", "default-folder"));
        assert_ne!(first, Uuid::derived(b"space", "other"));
        assert_eq!(first.to_string().as_bytes()[14], b'5');
    }

    #[test]
    fn refuses_anything_but_the_canonical_form() {
        for text in [
            "",
            "6ba7b810-9dad-11d1-80b4-00c04fd430c",
            "6ba7b810-9dad-11d1-80b4-00c04fd430c8-",
            "{6ba7b810-9dad-11d1-80b4-00c04fd430c8}",
            "6ba7b810-9dad-11d1-80b4-00c04fd430g8",
            "6ba7b8109-dad-11d1-80b4-00c04fd430c8",
        ] {
            assert!(Uuid::parse(text).is_none(), "{text}");
        }
    }

    #[test]
    fn orders_like_the_text() {
        let low = Uuid::parse("00000000-0000-4000-8000-000000000001").unwrap();
        let high = Uuid::parse("f0000000-0000-4000-8000-000000000000").unwrap();
        assert!(low < high);
        assert!(low.to_string() < high.to_string());
    }
}
