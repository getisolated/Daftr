//! Synchronisation entre appareils sur le réseau local : écoute TCP filtrée, canal Noise,
//! appairage SPAKE2 (code ou QR code), découverte mDNS, sessions multi-espaces, rattrapage
//! et temps réel. Sans SQL ni interface : le moteur ne parle qu'aux ports de `app`
//! (`SyncProfiles`, `SyncStore`, `SyncEvents`, `TrustStore`).
// Transport seul pour l'instant : le moteur qui l'utilise arrive dans un commit suivant
#![allow(dead_code)]

mod addr;
mod backoff;
mod clock;
mod error;
mod framing;
mod keys;
mod net;
mod noise;
mod wire;

pub use addr::AddrPolicy;
pub use backoff::BackoffPolicy;
pub use clock::{Clock, SystemClock};
pub use error::{Error, Result};
pub use framing::MAX_MESSAGE_BYTES;
pub use keys::{DeviceIdentity, Fingerprint, load_or_create_identity};
pub use wire::PROTOCOL_VERSION;
