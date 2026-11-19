//! Binaire de référence : runtime tokio + écoute TCP, aucune nouvelle crate.
//! Étalon des mesures de taille, RAM et CPU (voir `MESURES.md`).

fn main() -> anyhow::Result<()> {
    let port = sync_lan_spike::baseline::port_from_args();
    sync_lan_spike::baseline::runtime()?.block_on(sync_lan_spike::baseline::serve(port))
}
