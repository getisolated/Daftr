//! Côté B : découvre A (mDNS ou `IP:port`), se lie avec le code, échange la note.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use anyhow::{Context, bail, ensure};
use sync_lan_spike::addr_filter::is_local;
use sync_lan_spike::crdt::NoteDoc;
use sync_lan_spike::identity::{Identity, fingerprint};
use sync_lan_spike::pairing::pair_as_initiator;
use sync_lan_spike::transport::{Role, handshake};
use sync_lan_spike::{discovery, session};
use tokio::net::TcpStream;

use crate::cli::PairOptions;

const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(5);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

/// Cible de l'appairage : adresses à essayer et empreinte attendue si connue.
struct Target {
    addrs: Vec<SocketAddr>,
    fingerprint: Option<String>,
}

/// Se lie à A puis échange et fait converger la note.
pub async fn run(options: PairOptions) -> anyhow::Result<()> {
    let identity = Identity::generate()?;
    let target = resolve_target(&options).await?;
    let started = Instant::now();
    let stream = connect_first(&target.addrs).await?;

    let mut chan = handshake(stream, &identity, Role::Initiator).await?;
    let remote_fp = fingerprint(&chan.remote_static);
    if let Some(expected) = &target.fingerprint {
        ensure!(
            *expected == remote_fp,
            "empreinte inattendue : {remote_fp} au lieu de {expected}"
        );
    }
    println!(
        "[B] Noise XX ok en {:?}, pair {remote_fp}",
        started.elapsed()
    );

    if let Err(error) = pair_as_initiator(&mut chan, &identity.public, &options.code).await {
        println!("[B] appairage refusé : {error:#}");
        std::process::exit(1);
    }
    println!(
        "[B] code accepté, appareils liés en {:?}",
        started.elapsed()
    );
    sync_note(&mut chan, &identity).await?;
    println!("[B] terminé en {:?}", started.elapsed());
    Ok(())
}

async fn sync_note<S>(
    chan: &mut sync_lan_spike::transport::SecureChannel<S>,
    identity: &Identity,
) -> anyhow::Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let doc = NoteDoc::new(identity.yjs_client_id());
    session::exchange(chan, &doc).await?;
    println!("[B] rattrapage initial : {:?}", doc.content());

    let mut edited = doc.content().replacen(" :\n", " :\r\n", 1);
    edited.push_str("- œufs 🥚\n");
    doc.set_content(&edited);
    session::exchange(chan, &doc).await?;

    let content = session::verify_convergence(chan, &doc).await?;
    println!("[B] convergence ok, contenu {content:?}");
    Ok(())
}

async fn resolve_target(options: &PairOptions) -> anyhow::Result<Target> {
    let fingerprint = options.fingerprint.clone();
    if let Some(addr) = options.addr {
        return Ok(Target {
            addrs: vec![addr],
            fingerprint,
        });
    }
    let found =
        tokio::task::spawn_blocking(|| discovery::find_pairing_peer(DISCOVERY_TIMEOUT)).await??;
    let peer = found.context("aucun appareil en mode appairage trouvé par mDNS")?;
    println!(
        "[B] mDNS : appareil {} sur {:?}",
        peer.fingerprint, peer.addrs
    );
    Ok(Target {
        addrs: peer.addrs,
        fingerprint: fingerprint.or(Some(peer.fingerprint)),
    })
}

/// N'essaie que des adresses locales : le moteur ne compose jamais une IP publique.
async fn connect_first(addrs: &[SocketAddr]) -> anyhow::Result<TcpStream> {
    for addr in addrs {
        if !is_local(addr.ip()) {
            println!("[B] {addr} ignorée : adresse non locale");
            continue;
        }
        match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr)).await {
            Ok(Ok(stream)) => {
                println!("[B] connecté à {addr}");
                return Ok(stream);
            }
            Ok(Err(error)) => println!("[B] {addr} : {error}"),
            Err(_) => println!("[B] {addr} : délai dépassé"),
        }
    }
    bail!("aucune adresse joignable")
}
