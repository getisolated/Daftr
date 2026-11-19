//! Mesure de taille : binaire de référence + usage réaliste de `similar` seule.
//! Surcoût marginal = taille de ce binaire − taille de `reference` (voir `MESURES.md`).

use similar::TextDiff;
use sync_lan_spike::baseline;

fn main() -> anyhow::Result<()> {
    // Entrée lue à l'exécution pour que LTO ne puisse pas tout pré-calculer.
    let input = std::env::args().nth(2).unwrap_or_default();
    exercise(&input)?;
    baseline::runtime()?.block_on(baseline::serve(baseline::port_from_args()))
}

fn exercise(input: &str) -> anyhow::Result<()> {
    let changed = input.replace('a', "b");
    let diff = TextDiff::configure().diff_chars(input, changed.as_str());
    for op in diff.ops() {
        println!("{:?}", op.as_tag_tuple());
    }
    Ok(())
}
