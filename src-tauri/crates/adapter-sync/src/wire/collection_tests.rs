//! Compatibilité du protocole : messages de collections, `Hello` avec fonctionnalités, version
//! antérieure.

use super::*;

fn hello(kind: DeviceKind) -> Hello {
    Hello {
        protocol: PROTOCOL_VERSION,
        name: "Bureau".to_owned(),
        spaces: vec![[1; 32]],
        kind,
        features: FEATURE_COLLECTIONS,
    }
}

/// `Hello` tel que l'écrivait la version avec type d'appareil, sans fonctionnalités.
#[derive(Debug, PartialEq, Serialize, Deserialize)]
struct KindHello {
    protocol: u32,
    name: String,
    spaces: Vec<[u8; 32]>,
    kind: u8,
}

/// Indice de la variante `Hello` dans [`Message`].
const HELLO_VARIANT: u8 = 4;

#[test]
fn hello_advertises_the_collections_feature() {
    let bytes = Message::Hello(hello(DeviceKind::Laptop)).encode().unwrap();
    let Message::Hello(decoded) = Message::decode(&bytes).unwrap() else {
        panic!("Hello attendu");
    };
    assert_eq!(decoded.features, FEATURE_COLLECTIONS);
    assert_eq!(decoded.kind, DeviceKind::Laptop);
}

#[test]
fn a_hello_without_features_reads_as_having_none() {
    let before = KindHello {
        protocol: PROTOCOL_VERSION,
        name: "Ancien".to_owned(),
        spaces: vec![],
        kind: 1,
    };
    let mut bytes = vec![HELLO_VARIANT];
    bytes.extend(postcard::to_allocvec(&before).unwrap());

    let Message::Hello(hello) = Message::decode(&bytes).unwrap() else {
        panic!("Hello attendu");
    };
    assert_eq!((hello.kind, hello.features), (DeviceKind::Laptop, 0));
}

#[test]
fn an_older_build_still_reads_the_hello_with_features() {
    let bytes = Message::Hello(hello(DeviceKind::Desktop)).encode().unwrap();
    let before: KindHello = postcard::from_bytes(&bytes[1..]).unwrap();
    assert_eq!((before.kind, before.name.as_str()), (2, "Bureau"));
}

#[test]
fn a_message_from_a_newer_version_is_unknown_not_malformed() {
    for first in [VARIANT_COUNT, VARIANT_COUNT + 1, 127, 128, 255] {
        let bytes = [first, 1, 2, 3];
        assert!(matches!(
            Message::decode(&bytes),
            Err(Error::UnknownMessage)
        ));
    }
}

#[test]
fn collection_messages_round_trip_with_frozen_indexes() {
    let entry = WireEntry {
        path: "docs/a.md".to_owned(),
        hash: Some([7; 32]),
        size: 12,
        base: None,
    };
    let share = [2; 16];
    let path = || "a.md".to_owned();
    let messages = [
        Message::CollectionOffer(vec![OfferedCollection {
            space: Some([1; 32]),
            profile_id: "work".to_owned(),
            profile_name: "Travail".to_owned(),
            share,
            name: "Docs".to_owned(),
            files: 3,
            bytes: 4096,
        }]),
        Message::CollectionReply {
            share,
            accepted: true,
        },
        Message::Manifest {
            share,
            entries: vec![entry],
            last: true,
        },
        Message::FileRequest {
            share,
            path: path(),
            hash: [3; 32],
        },
        Message::FileChunk {
            share,
            path: path(),
            hash: [3; 32],
            total: 5,
            offset: 0,
            data: vec![1, 2, 3, 4, 5],
        },
        Message::FileUnavailable {
            share,
            path: path(),
            hash: [3; 32],
        },
        Message::FileAck {
            share,
            path: path(),
            hash: None,
        },
        Message::ShareStop { share },
        Message::ProfileInfo(ProfileInfoEntry {
            space: [1; 32],
            name: Some("Travail".to_owned()),
            icon: Some(vec![1, 2, 3]),
        }),
        Message::CollectionPending {
            shares: vec![share],
        },
        Message::Removed,
    ];
    for (offset, message) in messages.into_iter().enumerate() {
        let bytes = message.encode().unwrap();
        assert_eq!(
            usize::from(bytes[0]),
            14 + offset,
            "indice figé de {message:?}"
        );
        assert_eq!(Message::decode(&bytes).unwrap(), message);
    }
    assert_eq!(VARIANT_COUNT, 25);
}

#[test]
fn a_received_path_is_validated_before_use() {
    let wire = |path: &str| WireEntry {
        path: path.to_owned(),
        hash: Some([1; 32]),
        size: 1,
        base: None,
    };
    assert!(file_entry(wire("docs/a.md"), &[]).is_some());
    let refused = [
        "../a.md",
        "/etc/a.md",
        "C:/a.md",
        "d\\a.md",
        "con.md",
        "a.txt",
        ".git/a.md",
    ];
    for bad in refused {
        assert!(file_entry(wire(bad), &[]).is_none(), "{bad}");
    }
}

#[test]
fn a_collection_name_received_is_cleaned() {
    let offer = OfferedCollection {
        space: None,
        profile_id: "w\u{7}ork\n".to_owned(),
        profile_name: format!(" Tra\u{7}vail{} ", "x".repeat(500)),
        share: [2; 16],
        name: format!(" ../Do\u{7}cs{} ", "x".repeat(500)),
        files: 0,
        bytes: 0,
    };
    let name = offer.clean_name();
    assert!(name.starts_with("..Docs"));
    assert!(!name.contains('/') && !name.contains('\\'));
    assert!(name.chars().count() <= 128);
    assert_eq!(offer.clean_profile_id(), "work");
    let profile = offer.clean_profile_name();
    assert!(profile.starts_with("Travail") && profile.chars().count() <= 64);
}

#[test]
fn the_collections_bit_is_not_the_one_of_the_first_unreleased_version() {
    // Le bit 0 annonçait des offres liées à l'espace d'un profil, au format abandonné
    assert_eq!(FEATURE_COLLECTIONS & 1, 0);
    assert_eq!(FEATURE_PROFILE_INFO & (FEATURE_COLLECTIONS | 1), 0);
}
