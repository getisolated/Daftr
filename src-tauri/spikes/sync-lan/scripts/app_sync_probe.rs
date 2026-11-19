// Mesure du surcoût réel dans l'application (ne pas committer dans l'app) :
// 1. copier src-tauri/, package.json, assets/ et frontend/dist/browser hors du dépôt ;
// 2. `cargo build --release -p desktop --locked` → taille de référence ;
// 3. copier ce fichier en crates/desktop/src/sync_probe.rs, ajouter `mod sync_probe;` et
//    `sync_probe::run_if_requested()?;` en tête de `main`, puis dans crates/desktop/Cargo.toml :
//    `sync-lan-spike = { path = "<chemin>/spikes/sync-lan" }` et `tokio.workspace = true` ;
// 4. `cargo build --release -p desktop` → différence de taille de desktop.exe.

//! Sonde de mesure (copie jetable) : relie toute la pile du spike au binaire de l'app.

use std::time::Duration;

use sync_lan_spike::transport::{Role, handshake};
use sync_lan_spike::{crdt::NoteDoc, discovery, identity::Identity, pairing, qr, session};

/// Exécute la pile complète si `DAFTR_SYNC_PROBE` est défini (jamais en usage normal).
pub fn run_if_requested() -> anyhow::Result<()> {
    let Ok(input) = std::env::var("DAFTR_SYNC_PROBE") else { return Ok(()) };
    tauri::async_runtime::block_on(probe(&input))
}

async fn probe(input: &str) -> anyhow::Result<()> {
    let daemon = discovery::announce(input, 47820, input, None)?;
    println!("{:?}", discovery::find_pairing_peer(Duration::from_millis(10))?);
    drop(daemon);

    let (a, b) = tokio::io::duplex(1 << 16);
    let (ia, ib) = (Identity::generate()?, Identity::generate()?);
    let (ca, cb) = tokio::join!(handshake(a, &ia, Role::Initiator), handshake(b, &ib, Role::Responder));
    let (mut ca, mut cb) = (ca?, cb?);
    let (pa, pb) = tokio::join!(
        pairing::pair_as_initiator(&mut ca, &ia.public, input),
        pairing::pair_as_responder(&mut cb, &ib.public, input)
    );
    pa?;
    println!("{:?}", pb?);

    let (da, db) = (NoteDoc::new(ia.yjs_client_id()), NoteDoc::new(ib.yjs_client_id()));
    da.set_content(input);
    let (ra, rb) = tokio::join!(session::exchange(&mut ca, &da), session::exchange(&mut cb, &db));
    ra?;
    rb?;
    let (va, vb) = tokio::join!(session::verify_convergence(&mut ca, &da), session::verify_convergence(&mut cb, &db));
    println!("{} {}", va?, vb?);
    println!("{}", qr::render_svg(input)?.len());
    Ok(())
}
