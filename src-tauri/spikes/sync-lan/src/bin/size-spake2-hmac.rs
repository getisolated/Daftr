//! Mesure de taille : binaire de référence + usage réaliste de `spake2-hmac` seule.
//! Surcoût marginal = taille de ce binaire − taille de `reference` (voir `MESURES.md`).

use hmac::{Hmac, Mac};
use sha2::Sha256;
use spake2::{Ed25519Group, Identity, Password, Spake2};
use sync_lan_spike::baseline;

fn main() -> anyhow::Result<()> {
    // Entrée lue à l'exécution pour que LTO ne puisse pas tout pré-calculer.
    let input = std::env::args().nth(2).unwrap_or_default();
    exercise(&input)?;
    baseline::runtime()?.block_on(baseline::serve(baseline::port_from_args()))
}

fn exercise(input: &str) -> anyhow::Result<()> {
    let (ida, idb) = (Identity::new(b"a"), Identity::new(b"b"));
    let (sa, ma) = Spake2::<Ed25519Group>::start_a(&Password::new(input), &ida, &idb);
    let (sb, mb) = Spake2::<Ed25519Group>::start_b(&Password::new(input), &ida, &idb);
    let ka = sa.finish(&mb).map_err(|e| anyhow::anyhow!("{e}"))?;
    let kb = sb.finish(&ma).map_err(|e| anyhow::anyhow!("{e}"))?;

    let mut mac = Hmac::<Sha256>::new_from_slice(&ka)?;
    mac.update(input.as_bytes());
    let tag = mac.finalize().into_bytes();
    let mut check = Hmac::<Sha256>::new_from_slice(&kb)?;
    check.update(input.as_bytes());
    println!("{}", check.verify_slice(&tag).is_ok());
    Ok(())
}
