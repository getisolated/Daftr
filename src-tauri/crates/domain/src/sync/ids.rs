//! Identifiants échangés par la synchronisation : espaces, appareils, documents.

use std::fmt;

use crate::hex;
use crate::uuid::Uuid;

/// Déclare un identifiant de 32 octets écrit en hexadécimal minuscule (64 caractères).
macro_rules! key_type {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name([u8; 32]);

        impl $name {
            /// Les 32 octets tels quels.
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            /// Les 32 octets.
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }

            /// La valeur écrite en 64 chiffres hexadécimaux, `None` sinon.
            pub fn parse(text: &str) -> Option<Self> {
                hex::decode::<32>(text).map(Self)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&hex::encode(&self.0))
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(formatter, "{}({self})", stringify!($name))
            }
        }
    };
}

key_type! {
    /// Espace de synchronisation : 32 octets aléatoires, partagés par les profils synchronisés
    /// ensemble. Il n'est révélé qu'aux appareils membres de l'espace.
    SpaceId
}

key_type! {
    /// Clé publique statique (x25519) d'un appareil : c'est son identifiant.
    DeviceKey
}

/// Document CRDT d'un profil synchronisé.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DocId {
    /// Le document de l'espace : nom proposé du profil, dossiers, appareils.
    Space,
    /// Le document d'une note.
    Note(Uuid),
}

impl DocId {
    const SPACE: &'static str = "space";
    const NOTE_PREFIX: &'static str = "note:";

    /// L'identifiant écrit `space` ou `note:<uuid>`, `None` sinon.
    ///
    /// ```
    /// use domain::sync::DocId;
    ///
    /// let id = DocId::parse("note:6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap();
    /// assert_eq!(id.to_string(), "note:6ba7b810-9dad-11d1-80b4-00c04fd430c8");
    /// assert_eq!(DocId::parse("space"), Some(DocId::Space));
    /// assert_eq!(DocId::parse("note:42"), None);
    /// ```
    pub fn parse(text: &str) -> Option<Self> {
        if text == Self::SPACE {
            return Some(Self::Space);
        }
        let uuid = text.strip_prefix(Self::NOTE_PREFIX)?;
        Uuid::parse(uuid).map(Self::Note)
    }
}

impl fmt::Display for DocId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Space => formatter.write_str(Self::SPACE),
            Self::Note(uuid) => write!(formatter, "{}{uuid}", Self::NOTE_PREFIX),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip_through_hex() {
        let key = DeviceKey::from_bytes([0xAB; 32]);
        let text = key.to_string();
        assert_eq!(text, "ab".repeat(32));
        assert_eq!(DeviceKey::parse(&text), Some(key));
        assert_eq!(
            SpaceId::parse(&text.to_uppercase()),
            Some(SpaceId::from_bytes([0xAB; 32]))
        );
    }

    #[test]
    fn keys_refuse_other_lengths() {
        assert_eq!(SpaceId::parse(&"ab".repeat(31)), None);
        assert_eq!(SpaceId::parse(&"ab".repeat(33)), None);
        assert_eq!(DeviceKey::parse(""), None);
    }

    #[test]
    fn note_ids_need_a_valid_uuid() {
        assert_eq!(DocId::parse("note:"), None);
        assert_eq!(DocId::parse("Space"), None);
        assert_eq!(
            DocId::parse("folder:6ba7b810-9dad-11d1-80b4-00c04fd430c8"),
            None
        );
    }
}
