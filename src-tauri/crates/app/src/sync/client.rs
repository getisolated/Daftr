//! Identifiant client Yjs de l'appareil et création des documents.

use domain::sync::DeviceKey;
use yrs::{ClientID, Doc, OffsetKind, Options};

/// Les identifiants client Yjs tiennent sur 53 bits (entiers sûrs en JavaScript).
const CLIENT_ID_MASK: u64 = (1 << 53) - 1;

/// Identifiant client Yjs de l'appareil : les 53 bits de poids faible des 8 premiers octets
/// (petit-boutiste) de sa clé publique.
///
/// Il est déterministe, donc un appareil reprend toujours le même identifiant et continue
/// l'horloge de ses opérations passées (relue depuis le blob). La clé x25519 étant aléatoire,
/// deux appareils d'un même espace n'ont le même identifiant qu'avec une probabilité de
/// l'ordre de `n² / 2⁵⁴` pour `n` appareils.
pub(crate) fn client_id(device: &DeviceKey) -> ClientID {
    let mut prefix = [0_u8; 8];
    prefix.copy_from_slice(&device.as_bytes()[..8]);

    ClientID::new(u64::from_le_bytes(prefix) & CLIENT_ID_MASK)
}

/// Un document vide de l'appareil : offsets de texte en unités UTF-16 (ceux de l'éditeur et
/// de Yjs côté JavaScript) et ramasse-miettes actif, pour que le blob ne garde pas
/// l'historique des suppressions.
pub(crate) fn new_doc(device: &DeviceKey) -> Doc {
    let options = Options {
        offset_kind: OffsetKind::Utf16,
        skip_gc: false,
        ..Options::with_client_id(client_id(device))
    };
    Doc::with_options(options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_a_stable_53_bit_identifier_from_the_key() {
        let mut bytes = [0xFF_u8; 32];
        bytes[0] = 0x01;
        let device = DeviceKey::from_bytes(bytes);

        let id = client_id(&device).get();
        assert_eq!(id, client_id(&device).get());
        assert_eq!(id, CLIENT_ID_MASK & !0xFE);
        assert!(id < 1 << 53);
    }

    #[test]
    fn depends_only_on_the_first_bytes() {
        let mut other = [0xFF_u8; 32];
        other[31] = 0;
        assert_eq!(
            client_id(&DeviceKey::from_bytes([0xFF; 32])),
            client_id(&DeviceKey::from_bytes(other))
        );
        assert_ne!(
            client_id(&DeviceKey::from_bytes([1; 32])),
            client_id(&DeviceKey::from_bytes([2; 32]))
        );
    }

    #[test]
    fn documents_use_the_device_identifier() {
        let device = DeviceKey::from_bytes([3; 32]);
        assert_eq!(new_doc(&device).client_id(), client_id(&device));
    }
}
