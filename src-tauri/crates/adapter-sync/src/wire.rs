//! Messages du protocole, encodés en `postcard` dans le canal Noise.
//!
//! Le format est figé par l'ordre des variantes et des champs (postcard encode l'indice de
//! variante en varint) : on n'ajoute qu'en fin d'énumération, et on change
//! [`PROTOCOL_VERSION`] pour toute autre évolution. Seule exception : le dernier champ de
//! [`Hello`], lu de façon tolérante (voir [`Hello`]). Les identifiants voyagent en octets
//! (`SpaceId`) ou en texte (`DocId` : `space` ou `note:<uuid>`) et sont validés à la réception.

use std::fmt;

use app::ports::sync::ProfileOffer;
use app::sync::{StateVector, Update};
use domain::sync::{DeviceKind, DeviceName, DocId, SpaceId};
use serde::de::{self, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{Error, Result};

/// Version du protocole, annoncée dans `Hello` et dans le champ TXT mDNS.
pub const PROTOCOL_VERSION: u32 = 1;

/// Documents au plus par message de rattrapage (et par transaction côté stockage).
pub(crate) const BATCH_DOCS: usize = 200;
/// Volume visé d'un message de diffs : bien sous le maximum, pour garder des messages courts.
pub(crate) const BATCH_BYTES: usize = 4 * 1024 * 1024;
/// Profils au plus dans une offre.
pub(crate) const MAX_OFFERS: usize = 64;
/// Longueur maximale d'un nom de profil proposé, en caractères.
const MAX_OFFER_NAME_CHARS: usize = 128;
/// Espaces au plus dans un `Hello`.
pub(crate) const MAX_SPACES: usize = 1024;

/// Type de session, premier message de l'appareil qui compose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum SessionKind {
    /// Appairage par code, suivi d'une session de synchronisation.
    Pair,
    /// Synchronisation entre appareils de confiance.
    Sync,
}

/// Cause d'un refus d'appairage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum PairRefusal {
    /// L'appareil n'est pas en mode appairage (ou le mode a expiré).
    NotPairing,
    /// La confirmation de clé ne correspond pas : mauvais code.
    WrongCode,
}

/// État d'un document chez l'émetteur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DocState {
    /// `space` ou `note:<uuid>`.
    pub doc: String,
    /// State vector Yjs (v1).
    pub sv: Vec<u8>,
}

/// Diff d'un document, avec l'état de l'émetteur.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DocDiff {
    /// `space` ou `note:<uuid>`.
    pub doc: String,
    /// Update Yjs (v1) : ce que l'émetteur sait et que le destinataire ignore.
    pub update: Vec<u8>,
    /// State vector de l'émetteur.
    pub sv: Vec<u8>,
}

/// Update d'un document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DocUpdate {
    /// `space` ou `note:<uuid>`.
    pub doc: String,
    /// Update Yjs (v1).
    pub update: Vec<u8>,
}

/// Profil proposé.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct OfferEntry {
    /// Espace proposé.
    pub space: [u8; 32],
    /// Nom proposé du profil.
    pub name: String,
    /// Nombre de notes.
    pub notes: u64,
}

/// Annonce d'une session : nom, type et espaces que l'émetteur accepte d'échanger avec le
/// destinataire (ceux où le destinataire est membre, synchronisation activée).
///
/// `kind` a été ajouté en fin de message sans changer [`PROTOCOL_VERSION`] : une version
/// antérieure ignore ces octets en trop (postcard ne vérifie pas la fin du message), et un
/// `Hello` sans eux, ou avec un code inconnu (type ajouté plus tard), se lit `Unknown`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Hello {
    /// Version du protocole de l'émetteur.
    pub protocol: u32,
    /// Nom de l'émetteur.
    pub name: String,
    /// Espaces annoncés.
    pub spaces: Vec<[u8; 32]>,
    /// Type de l'émetteur, en code d'un octet.
    #[serde(serialize_with = "serialize_kind")]
    pub kind: DeviceKind,
}

/// Champs de [`Hello`].
const HELLO_FIELDS: usize = 4;

/// Codes des types d'appareil : figés, un nouveau type prend le code suivant.
const KIND_UNKNOWN: u8 = 0;
const KIND_LAPTOP: u8 = 1;
const KIND_DESKTOP: u8 = 2;

fn kind_code(kind: DeviceKind) -> u8 {
    match kind {
        DeviceKind::Unknown => KIND_UNKNOWN,
        DeviceKind::Laptop => KIND_LAPTOP,
        DeviceKind::Desktop => KIND_DESKTOP,
    }
}

fn kind_from_code(code: u8) -> DeviceKind {
    match code {
        KIND_LAPTOP => DeviceKind::Laptop,
        KIND_DESKTOP => DeviceKind::Desktop,
        _ => DeviceKind::Unknown,
    }
}

fn serialize_kind<S: Serializer>(
    kind: &DeviceKind,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error> {
    serializer.serialize_u8(kind_code(*kind))
}

impl<'de> Deserialize<'de> for Hello {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        deserializer.deserialize_tuple(HELLO_FIELDS, HelloVisitor)
    }
}

struct HelloVisitor;

impl<'de> Visitor<'de> for HelloVisitor {
    type Value = Hello;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Hello")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> std::result::Result<Hello, A::Error> {
        let protocol = seq
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(0, &self))?;
        let name = seq
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(1, &self))?;
        let spaces = seq
            .next_element()?
            .ok_or_else(|| de::Error::invalid_length(2, &self))?;
        // Fin du message atteinte : `Hello` d'une version antérieure, sans type
        let kind = seq.next_element::<u8>().ok().flatten();
        Ok(Hello {
            protocol,
            name,
            spaces,
            kind: kind.map_or(DeviceKind::Unknown, kind_from_code),
        })
    }
}

/// Tous les messages, dans l'ordre figé du format.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Message {
    /// Premier message de l'appareil qui compose.
    Open(SessionKind),
    /// Message SPAKE2.
    Spake(Vec<u8>),
    /// Confirmation de clé (HMAC sur le hash du handshake).
    Confirm(Vec<u8>),
    /// Appairage refusé.
    Refused(PairRefusal),
    /// Annonce (ou nouvelle annonce) des espaces.
    Hello(Hello),
    /// Rattrapage : documents de l'émetteur modifiés depuis le dernier acquittement.
    Changed {
        /// Espace.
        space: [u8; 32],
        /// Documents et leur state vector.
        docs: Vec<DocState>,
    },
    /// Réponse à `Changed` : diffs du destinataire ; `last` clôt la réponse.
    Diffs {
        /// Espace.
        space: [u8; 32],
        /// Diffs.
        docs: Vec<DocDiff>,
        /// Dernier message de la réponse.
        last: bool,
    },
    /// Diffs de l'émetteur de `Changed` en retour.
    Updates {
        /// Espace.
        space: [u8; 32],
        /// Updates.
        docs: Vec<DocUpdate>,
    },
    /// Fin d'un rattrapage : tout jusqu'à `seq` a été envoyé.
    Done {
        /// Espace.
        space: [u8; 32],
        /// `seq` local de l'émetteur.
        seq: u64,
    },
    /// Acquittement : tout jusqu'à `seq` a été appliqué.
    Ack {
        /// Espace.
        space: [u8; 32],
        /// `seq` acquitté.
        seq: u64,
    },
    /// Profils proposés.
    Offer(Vec<OfferEntry>),
    /// Réponse à une offre.
    OfferReply {
        /// Espace proposé.
        space: [u8; 32],
        /// Offre acceptée.
        accepted: bool,
    },
    /// Contenu d'un espace accepté, par morceaux.
    Snapshot {
        /// Espace.
        space: [u8; 32],
        /// Nombre total de documents.
        total: u64,
        /// Documents complets.
        docs: Vec<DocUpdate>,
    },
    /// Fin du contenu : tout jusqu'à `seq` a été envoyé.
    SnapshotEnd {
        /// Espace.
        space: [u8; 32],
        /// `seq` local de l'émetteur.
        seq: u64,
    },
}

impl Message {
    /// Encode le message.
    pub(crate) fn encode(&self) -> Result<Vec<u8>> {
        Ok(postcard::to_allocvec(self)?)
    }

    /// Décode un message reçu.
    pub(crate) fn decode(bytes: &[u8]) -> Result<Self> {
        Ok(postcard::from_bytes(bytes)?)
    }
}

/// L'identifiant d'un document reçu.
pub(crate) fn doc_id(text: &str) -> Result<DocId> {
    DocId::parse(text).ok_or(Error::Malformed)
}

/// Le state vector reçu.
pub(crate) fn state_vector(bytes: Vec<u8>) -> Result<StateVector> {
    StateVector::decode(bytes).map_err(|_| Error::Malformed)
}

/// L'update reçue (validée par le stockage à l'application).
pub(crate) fn update(bytes: Vec<u8>) -> Update {
    Update::from_bytes(bytes)
}

/// Le nom d'appareil reçu, assaini.
pub(crate) fn device_name(text: &str) -> Result<DeviceName> {
    DeviceName::sanitize(text).ok_or(Error::Malformed)
}

/// L'offre à envoyer.
pub(crate) fn offer_entry(offer: &ProfileOffer) -> OfferEntry {
    OfferEntry {
        space: *offer.space_id.as_bytes(),
        name: offer.name.chars().take(MAX_OFFER_NAME_CHARS).collect(),
        notes: offer.note_count,
    }
}

/// L'offre reçue, nom assaini (caractères de contrôle retirés, longueur bornée).
pub(crate) fn profile_offer(entry: &OfferEntry) -> ProfileOffer {
    let name = entry
        .name
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_OFFER_NAME_CHARS)
        .collect::<String>();
    ProfileOffer {
        space_id: SpaceId::from_bytes(entry.space),
        name: name.trim().to_owned(),
        note_count: entry.notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hello(kind: DeviceKind) -> Hello {
        Hello {
            protocol: PROTOCOL_VERSION,
            name: "Bureau".to_owned(),
            spaces: vec![[1; 32]],
            kind,
        }
    }

    /// `Hello` tel que l'écrivaient et le lisaient les versions sans type d'appareil.
    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct LegacyHello {
        protocol: u32,
        name: String,
        spaces: Vec<[u8; 32]>,
    }

    /// Indice de la variante `Hello` dans [`Message`].
    const HELLO_VARIANT: u8 = 4;

    #[test]
    fn hello_carries_the_device_kind() {
        for kind in [DeviceKind::Unknown, DeviceKind::Laptop, DeviceKind::Desktop] {
            let message = Message::Hello(hello(kind));
            let bytes = message.encode().unwrap();
            assert_eq!(bytes[0], HELLO_VARIANT);
            assert_eq!(Message::decode(&bytes).unwrap(), message);
        }
    }

    #[test]
    fn a_hello_without_kind_reads_as_unknown() {
        let legacy = LegacyHello {
            protocol: PROTOCOL_VERSION,
            name: "Ancien".to_owned(),
            spaces: vec![[3; 32]],
        };
        let mut bytes = vec![HELLO_VARIANT];
        bytes.extend(postcard::to_allocvec(&legacy).unwrap());

        let Message::Hello(hello) = Message::decode(&bytes).unwrap() else {
            panic!("Hello attendu");
        };
        assert_eq!(hello.kind, DeviceKind::Unknown);
        assert_eq!(
            (hello.protocol, hello.name, hello.spaces),
            (1, legacy.name, legacy.spaces)
        );
    }

    #[test]
    fn an_older_build_still_reads_the_new_hello() {
        let bytes = Message::Hello(hello(DeviceKind::Desktop)).encode().unwrap();
        let legacy: LegacyHello = postcard::from_bytes(&bytes[1..]).unwrap();
        assert_eq!(legacy.name, "Bureau");
        assert_eq!(legacy.protocol, PROTOCOL_VERSION);
    }

    #[test]
    fn an_unknown_kind_code_reads_as_unknown() {
        let mut bytes = Message::Hello(hello(DeviceKind::Laptop)).encode().unwrap();
        if let Some(last) = bytes.last_mut() {
            *last = 9;
        }
        let Message::Hello(hello) = Message::decode(&bytes).unwrap() else {
            panic!("Hello attendu");
        };
        assert_eq!(hello.kind, DeviceKind::Unknown);
    }

    #[test]
    fn messages_round_trip() {
        let messages = [
            Message::Open(SessionKind::Sync),
            Message::Hello(hello(DeviceKind::Laptop)),
            Message::Diffs {
                space: [2; 32],
                docs: vec![DocDiff {
                    doc: "space".to_owned(),
                    update: vec![1, 2],
                    sv: vec![0],
                }],
                last: true,
            },
            Message::Refused(PairRefusal::WrongCode),
        ];
        for message in messages {
            let bytes = message.encode().unwrap();
            assert_eq!(Message::decode(&bytes).unwrap(), message);
        }
    }

    #[test]
    fn the_variant_order_is_frozen() {
        assert_eq!(Message::Open(SessionKind::Pair).encode().unwrap(), [0, 0]);
        assert_eq!(Message::Open(SessionKind::Sync).encode().unwrap(), [0, 1]);
        let ack = Message::Ack {
            space: [0; 32],
            seq: 1,
        };
        assert_eq!(ack.encode().unwrap()[0], 9);
        let end = Message::SnapshotEnd {
            space: [0; 32],
            seq: 1,
        };
        assert_eq!(end.encode().unwrap()[0], 13);
    }

    #[test]
    fn garbage_is_malformed_not_a_panic() {
        for bytes in [&[][..], &[200], &[5, 255, 255, 255, 255, 255]] {
            assert!(matches!(Message::decode(bytes), Err(Error::Malformed)));
        }
        assert!(doc_id("note:42").is_err());
        assert!(device_name("\n").is_err());
    }

    #[test]
    fn received_offers_are_sanitized() {
        let entry = OfferEntry {
            space: [9; 32],
            name: format!(" Pro\u{7}{}", "x".repeat(500)),
            notes: 3,
        };
        let offer = profile_offer(&entry);
        assert!(offer.name.starts_with("Pro"));
        assert!(offer.name.chars().count() <= MAX_OFFER_NAME_CHARS);
        assert_eq!(offer.space_id, SpaceId::from_bytes([9; 32]));
    }
}
