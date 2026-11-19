//! Mesure de taille : binaire de référence + usage réaliste de `mdns-sd` seule.
//! Surcoût marginal = taille de ce binaire − taille de `reference` (voir `MESURES.md`).

use std::time::Duration;
use sync_lan_spike::baseline;

use sync_lan_spike::discovery;

fn main() -> anyhow::Result<()> {
    // Entrée lue à l'exécution pour que LTO ne puisse pas tout pré-calculer.
    let input = std::env::args().nth(2).unwrap_or_default();
    exercise(&input)?;
    baseline::runtime()?.block_on(baseline::serve(baseline::port_from_args()))
}

fn exercise(input: &str) -> anyhow::Result<()> {
    let daemon = discovery::announce(input, 47821, input, None)?;
    println!(
        "{:?}",
        discovery::find_pairing_peer(Duration::from_millis(input.len() as u64))?
    );
    drop(daemon);
    Ok(())
}
