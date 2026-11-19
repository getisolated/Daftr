//! Spike jetable du lot 0 de la synchronisation LAN (voir `specs/sync-lan.md`).
//!
//! Valide la pile `mdns-sd` + `snow` + `spake2` + `yrs` + `similar` + `qrcode`
//! et sert de support aux mesures consignées dans `MESURES.md`.

pub mod addr_filter;
pub mod baseline;
pub mod crdt;
pub mod discovery;
pub mod identity;
pub mod message;
pub mod pairing;
pub mod qr;
pub mod session;
pub mod transport;
