//! Mesure de taille : binaire de référence + usage réaliste de `yrs` seule.
//! Surcoût marginal = taille de ce binaire − taille de `reference` (voir `MESURES.md`).

use sync_lan_spike::baseline;
use yrs::updates::decoder::Decode;
use yrs::updates::encoder::Encode;
use yrs::{
    ClientID, Doc, GetString, OffsetKind, Options, ReadTxn, StateVector, Text, Transact, Update,
};

fn main() -> anyhow::Result<()> {
    // Entrée lue à l'exécution pour que LTO ne puisse pas tout pré-calculer.
    let input = std::env::args().nth(2).unwrap_or_default();
    exercise(&input)?;
    baseline::runtime()?.block_on(baseline::serve(baseline::port_from_args()))
}

fn exercise(input: &str) -> anyhow::Result<()> {
    let options = |id| Options {
        offset_kind: OffsetKind::Utf16,
        ..Options::with_client_id(ClientID::new(id))
    };
    let (a, b) = (Doc::with_options(options(1)), Doc::with_options(options(2)));
    let (ta, tb) = (
        a.get_or_insert_text("content"),
        b.get_or_insert_text("content"),
    );
    {
        let mut txn = a.transact_mut();
        ta.insert(&mut txn, 0, input);
        ta.remove_range(&mut txn, 0, (input.len() / 2) as u32);
    }
    let sv = b.transact().state_vector().encode_v1();
    let diff = a.transact().encode_diff_v1(&StateVector::decode_v1(&sv)?);
    b.transact_mut().apply_update(Update::decode_v1(&diff)?)?;
    println!("{}", tb.get_string(&b.transact()));
    Ok(())
}
