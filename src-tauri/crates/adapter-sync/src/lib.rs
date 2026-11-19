//! Synchronisation entre appareils sur le réseau local : écoute TCP filtrée, canal Noise,
//! appairage SPAKE2 (code ou QR code), découverte mDNS, sessions multi-espaces, rattrapage
//! et temps réel. Sans SQL ni interface : le moteur ne parle qu'aux ports de `app`
//! (`SyncProfiles`, `SyncStore`, `SyncEvents`, `TrustStore`).
// Briques seules pour l'instant : le moteur qui les utilise arrive dans un commit suivant
#![allow(dead_code, unused_imports)]

mod addr;
mod backoff;
mod clock;
mod discovery;
mod error;
mod framing;
mod keys;
mod net;
mod noise;
mod pairing;
mod pake;
mod qr;
mod wire;

pub use addr::AddrPolicy;
pub use backoff::BackoffPolicy;
pub use clock::{Clock, SystemClock};
pub use discovery::{Announcement, Discovery, MdnsDiscovery, SERVICE_TYPE, Sighting};
#[cfg(any(test, feature = "testing"))]
pub use discovery::{MemoryDiscovery, MemoryNetwork};
pub use error::{Error, Result};
pub use framing::MAX_MESSAGE_BYTES;
pub use keys::{DeviceIdentity, Fingerprint, load_or_create_identity};
pub use pairing::{LinkedDevice, MAX_FAILURES, PAIRING_DURATION, PairingState, PairingTicket};
pub use qr::{MAX_QR_ADDRS, PairingUri};
pub use wire::PROTOCOL_VERSION;
