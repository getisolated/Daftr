//! Mesure de taille : binaire de référence + usage réaliste de `postcard` seule.
//! Surcoût marginal = taille de ce binaire − taille de `reference` (voir `MESURES.md`).

use sync_lan_spike::baseline;
use sync_lan_spike::message::{Message, SessionKind};

fn main() -> anyhow::Result<()> {
    // Entrée lue à l'exécution pour que LTO ne puisse pas tout pré-calculer.
    let input = std::env::args().nth(2).unwrap_or_default();
    exercise(&input)?;
    baseline::runtime()?.block_on(baseline::serve(baseline::port_from_args()))
}

fn exercise(input: &str) -> anyhow::Result<()> {
    let messages = [
        Message::Session(SessionKind::Pair),
        Message::Spake(input.as_bytes().to_vec()),
        Message::Content(input.to_owned()),
    ];
    for message in &messages {
        let bytes = postcard::to_allocvec(message)?;
        println!("{:?}", postcard::from_bytes::<Message>(&bytes)?);
    }
    Ok(())
}
