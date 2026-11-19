//! Mesure de taille : binaire de référence + usage réaliste de `snow` seule.
//! Surcoût marginal = taille de ce binaire − taille de `reference` (voir `MESURES.md`).

use sync_lan_spike::baseline;
use sync_lan_spike::identity::{Identity, NOISE_PARAMS};

fn main() -> anyhow::Result<()> {
    // Entrée lue à l'exécution pour que LTO ne puisse pas tout pré-calculer.
    let input = std::env::args().nth(2).unwrap_or_default();
    exercise(&input)?;
    baseline::runtime()?.block_on(baseline::serve(baseline::port_from_args()))
}

fn exercise(input: &str) -> anyhow::Result<()> {
    let (a, b) = (Identity::generate()?, Identity::generate()?);
    let params = || NOISE_PARAMS.parse::<snow::params::NoiseParams>();
    let mut init = snow::Builder::new(params()?)
        .local_private_key(&a.private)?
        .build_initiator()?;
    let mut resp = snow::Builder::new(params()?)
        .local_private_key(&b.private)?
        .build_responder()?;
    let (mut buf, mut out) = (vec![0u8; 65535], vec![0u8; 65535]);

    let n = init.write_message(&[], &mut buf)?;
    resp.read_message(&buf[..n], &mut out)?;
    let n = resp.write_message(&[], &mut buf)?;
    init.read_message(&buf[..n], &mut out)?;
    let n = init.write_message(&[], &mut buf)?;
    resp.read_message(&buf[..n], &mut out)?;

    let (mut ti, mut tr) = (init.into_transport_mode()?, resp.into_transport_mode()?);
    let n = ti.write_message(input.as_bytes(), &mut buf)?;
    let m = tr.read_message(&buf[..n], &mut out)?;
    println!("{}", m);
    Ok(())
}
