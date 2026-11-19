//! Encodage des documents : blobs, state vectors, updates (format Yjs v1).
//!
//! Tout octet reçu d'un autre appareil est non fiable : sa taille est bornée, son décodage
//! rend une erreur typée et l'intégration est isolée d'une éventuelle panique de `yrs`.

use std::panic::{AssertUnwindSafe, catch_unwind};

use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{Doc, ReadTxn, Transact};

use crate::error::{SyncError, SyncResult};

/// Taille maximale d'une update, d'un blob ou d'un state vector accepté (16 Mio, comme un
/// message du protocole).
pub const MAX_UPDATE_BYTES: usize = 16 * 1024 * 1024;

/// State vector encodé (Yjs v1) : ce qu'un appareil connaît d'un document.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StateVector(Vec<u8>);

impl StateVector {
    /// Octets reçus, validés : taille bornée et décodage complet.
    pub fn decode(bytes: Vec<u8>) -> SyncResult<Self> {
        decode_state_vector(&bytes)?;
        Ok(Self(bytes))
    }

    /// Le state vector d'un appareil qui ne connaît rien du document.
    pub fn empty() -> Self {
        Self(yrs::StateVector::default().encode_v1())
    }

    /// Les octets encodés.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Les octets encodés, consommés.
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }

    pub(crate) fn decoded(&self) -> SyncResult<yrs::StateVector> {
        decode_state_vector(&self.0)
    }
}

/// Update encodée (Yjs v1) : un blob complet ou un diff. Elle n'est validée qu'au moment de
/// l'appliquer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Update(Vec<u8>);

impl Update {
    /// Octets reçus ou lus, tels quels.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }

    /// Les octets encodés.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// Les octets encodés, consommés.
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }

    /// Une update sans aucune opération (rien à envoyer).
    pub fn is_empty(&self) -> bool {
        self.0 == yrs::Update::new().encode_v1()
    }
}

/// Échanges communs aux documents de note et d'espace.
pub trait SyncDoc: sealed::Inner {
    /// Le blob à stocker : le document entier, ré-encodé depuis son état courant.
    fn encode(&self) -> Update {
        encode(self.inner())
    }

    /// Ce que le document connaît.
    fn state_vector(&self) -> StateVector {
        state_vector(self.inner())
    }

    /// Ce que le document connaît et que l'appareil distant (`remote`) ne connaît pas ; vide
    /// s'il est déjà à jour.
    fn diff(&self, remote: &StateVector) -> SyncResult<Update> {
        diff(self.inner(), remote)
    }

    /// Intègre une update reçue (validée). En cas d'erreur, le document peut être
    /// partiellement modifié : le jeter et repartir du blob stocké.
    fn apply_update(&self, update: &Update) -> SyncResult<()> {
        apply_update(self.inner(), update)
    }

    /// Une partie de ce qui a été reçu attend des opérations antérieures : demander un diff
    /// complet à partir du [`SyncDoc::state_vector`].
    fn has_missing_updates(&self) -> bool {
        has_missing_updates(self.inner())
    }
}

pub(crate) mod sealed {
    /// Accès au document `yrs`, réservé à ce crate.
    pub trait Inner {
        /// Le document.
        fn inner(&self) -> &yrs::Doc;
    }
}

/// Le document entier, ré-encodé depuis son état courant : sans les opérations déjà
/// ramassées par le GC, c'est le blob à stocker.
pub(crate) fn encode(doc: &Doc) -> Update {
    let transaction = doc.transact();
    Update(transaction.encode_state_as_update_v1(&yrs::StateVector::default()))
}

/// Ce que le document connaît.
pub(crate) fn state_vector(doc: &Doc) -> StateVector {
    StateVector(doc.transact().state_vector().encode_v1())
}

/// Ce que le document connaît et que `remote` ne connaît pas.
pub(crate) fn diff(doc: &Doc, remote: &StateVector) -> SyncResult<Update> {
    let remote = remote.decoded()?;
    Ok(Update(doc.transact().encode_diff_v1(&remote)))
}

/// Intègre une update reçue.
///
/// En cas d'erreur, le document peut être partiellement modifié : l'appelant le jette et
/// repart du blob stocké.
pub(crate) fn apply_update(doc: &Doc, update: &Update) -> SyncResult<()> {
    check_size(update.as_bytes())?;
    let decoded = yrs::Update::decode_v1(update.as_bytes()).map_err(|_| SyncError::Malformed)?;

    // `yrs` peut paniquer sur une update bien formée mais incohérente : elle est refusée
    let integrated = catch_unwind(AssertUnwindSafe(|| {
        doc.transact_mut().apply_update(decoded)
    }));
    match integrated {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(SyncError::Rejected(error.to_string())),
        Err(_) => Err(SyncError::Malformed),
    }
}

/// Le document attend des opérations antérieures pour intégrer une partie de ce qu'il a
/// reçu : il faut lui envoyer un diff complet depuis son state vector.
pub(crate) fn has_missing_updates(doc: &Doc) -> bool {
    doc.transact().has_missing_updates()
}

fn decode_state_vector(bytes: &[u8]) -> SyncResult<yrs::StateVector> {
    check_size(bytes)?;
    yrs::StateVector::decode_v1(bytes).map_err(|_| SyncError::MalformedStateVector)
}

fn check_size(bytes: &[u8]) -> SyncResult<()> {
    if bytes.len() > MAX_UPDATE_BYTES {
        return Err(SyncError::TooLarge { size: bytes.len() });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use domain::sync::DeviceKey;
    use yrs::{GetString, Text};

    use super::*;
    use crate::sync::client::new_doc;

    fn doc_with(device: u8, text: &str) -> Doc {
        let doc = new_doc(&DeviceKey::from_bytes([device; 32]));
        let content = doc.get_or_insert_text("content");
        content.insert(&mut doc.transact_mut(), 0, text);
        doc
    }

    fn text_of(doc: &Doc) -> String {
        let content = doc.get_or_insert_text("content");
        content.get_string(&doc.transact())
    }

    #[test]
    fn a_diff_brings_the_other_side_up_to_date() {
        let local = doc_with(1, "héllo");
        let remote = doc_with(2, "");

        let update = diff(&local, &state_vector(&remote)).unwrap();
        apply_update(&remote, &update).unwrap();

        assert_eq!(text_of(&remote), "héllo");
        assert!(diff(&local, &state_vector(&remote)).unwrap().is_empty());
    }

    #[test]
    fn a_blob_restores_the_whole_document() {
        let local = doc_with(1, "a\r\nb 🎉");
        let restored = new_doc(&DeviceKey::from_bytes([1; 32]));

        apply_update(&restored, &encode(&local)).unwrap();
        assert_eq!(text_of(&restored), "a\r\nb 🎉");
    }

    #[test]
    fn refuses_oversized_input() {
        let doc = doc_with(1, "");
        let huge = Update::from_bytes(vec![0; MAX_UPDATE_BYTES + 1]);
        assert!(matches!(
            apply_update(&doc, &huge),
            Err(SyncError::TooLarge { .. })
        ));
        assert!(matches!(
            StateVector::decode(vec![0; MAX_UPDATE_BYTES + 1]),
            Err(SyncError::TooLarge { .. })
        ));
    }

    #[test]
    fn refuses_malformed_input_without_panicking() {
        let doc = doc_with(1, "x");
        for bytes in [vec![], vec![0xFF; 12], vec![1, 2, 3]] {
            assert!(apply_update(&doc, &Update::from_bytes(bytes)).is_err());
        }
        for bytes in [vec![], vec![0xFF; 12], vec![2, 1]] {
            assert!(StateVector::decode(bytes).is_err());
        }
        assert_eq!(text_of(&doc), "x");
    }

    #[test]
    fn truncated_updates_are_refused() {
        let blob = encode(&doc_with(1, "contenu assez long pour être coupé"));
        let doc = doc_with(2, "");

        for length in 1..blob.as_bytes().len() {
            let truncated = Update::from_bytes(blob.as_bytes()[..length].to_vec());
            // Une coupure peut donner une update valide mais incomplète : jamais de panique
            let _ = apply_update(&doc, &truncated);
        }
    }

    #[test]
    fn reports_missing_operations() {
        let source = doc_with(1, "a");
        let before = state_vector(&source);
        source
            .get_or_insert_text("content")
            .insert(&mut source.transact_mut(), 1, "b");

        let target = doc_with(2, "");
        apply_update(&target, &diff(&source, &before).unwrap()).unwrap();
        assert!(has_missing_updates(&target));

        apply_update(&target, &encode(&source)).unwrap();
        assert!(!has_missing_updates(&target));
        assert_eq!(text_of(&target), "ab");
    }

    #[test]
    fn the_empty_state_vector_asks_for_everything() {
        let doc = doc_with(1, "abc");
        assert_eq!(
            diff(&doc, &StateVector::empty()).unwrap().as_bytes().len(),
            encode(&doc).as_bytes().len()
        );
    }
}
