//! Mesure de taille : binaire de référence + usage réaliste de `qrcode` seule.
//! Surcoût marginal = taille de ce binaire − taille de `reference` (voir `MESURES.md`).

use sync_lan_spike::baseline;
use sync_lan_spike::qr;

fn main() -> anyhow::Result<()> {
    // Entrée lue à l'exécution pour que LTO ne puisse pas tout pré-calculer.
    let input = std::env::args().nth(2).unwrap_or_default();
    exercise(&input)?;
    baseline::runtime()?.block_on(baseline::serve(baseline::port_from_args()))
}

fn exercise(input: &str) -> anyhow::Result<()> {
    println!("{}", qr::render_svg(input)?.len());
    Ok(())
}
