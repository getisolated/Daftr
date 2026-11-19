//! Côté A : écoute TCP, filtre d'adresses, annonce mDNS, code et QR code.

use std::net::{IpAddr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use anyhow::Context;
use sync_lan_spike::addr_filter::is_local;
use sync_lan_spike::crdt::NoteDoc;
use sync_lan_spike::identity::{Identity, fingerprint, random_pairing_code};
use sync_lan_spike::pairing::{PairOutcome, pair_as_responder};
use sync_lan_spike::transport::{Role, handshake};
use sync_lan_spike::{discovery, qr, session};
use tokio::net::{TcpListener, TcpStream};

use crate::cli::ListenOptions;

/// Au-delà, un nouveau code est tiré (§6).
const MAX_FAILURES: u32 = 10;
/// Une connexion bloquée ne doit pas monopoliser l'écoute séquentielle.
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(30);
const INITIAL_NOTE: &str = "Liste de courses :\n- pain\n- lait\n";

struct Listener {
    identity: Identity,
    code: String,
    failures: u32,
}

/// Lance l'écoute ; ne rend la main qu'en cas d'erreur.
pub async fn run(options: ListenOptions) -> anyhow::Result<()> {
    let identity = Identity::generate()?;
    let code = options.code.clone().unwrap_or_else(random_pairing_code);
    let tcp = TcpListener::bind(("0.0.0.0", options.port))
        .await
        .context("écoute TCP")?;
    // Windows ouvre les sockets IPv6 en IPV6_V6ONLY : il faut une seconde écoute,
    // car mDNS annonce aussi les adresses IPv6 lien-local.
    let tcp6 = TcpListener::bind(("::", options.port)).await.ok();

    let lan_ip = primary_lan_ip().unwrap_or(IpAddr::from([127, 0, 0, 1]));
    let advertised = SocketAddr::new(lan_ip, options.port);
    println!(
        "[A] écoute sur {} et [::] : {} (IP LAN {lan_ip})",
        tcp.local_addr()?,
        tcp6.is_some()
    );
    println!("[A] empreinte {}", identity.fingerprint());
    println!("[A] code d'appairage {code}");
    write_qr(&options, advertised, &identity, &code)?;

    let _mdns = options
        .mdns
        .then(|| {
            discovery::announce(
                "daftr-spike",
                options.port,
                &identity.fingerprint(),
                options.ip_check_secs,
            )
        })
        .transpose()?;
    if options.mdns {
        println!("[A] annonce mDNS {} active", discovery::SERVICE_TYPE);
    }

    let mut state = Listener {
        identity,
        code,
        failures: 0,
    };
    accept_loop(&tcp, tcp6.as_ref(), &mut state).await
}

async fn accept_loop(
    tcp: &TcpListener,
    tcp6: Option<&TcpListener>,
    state: &mut Listener,
) -> anyhow::Result<()> {
    loop {
        let (stream, peer) = accept_any(tcp, tcp6).await?;
        if !is_local(peer.ip()) {
            println!("[A] {peer} refusée avant handshake : adresse non locale");
            continue;
        }
        match tokio::time::timeout(CONNECTION_TIMEOUT, state.serve(stream, peer)).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => println!("[A] {peer} : échec : {error:#}"),
            Err(_) => println!("[A] {peer} : délai dépassé"),
        }
    }
}

impl Listener {
    async fn serve(&mut self, stream: TcpStream, peer: SocketAddr) -> anyhow::Result<()> {
        let started = Instant::now();
        let mut chan = handshake(stream, &self.identity, Role::Responder).await?;
        println!(
            "[A] {peer} : Noise XX ok en {:?}, pair {}",
            started.elapsed(),
            fingerprint(&chan.remote_static)
        );

        if pair_as_responder(&mut chan, &self.identity.public, &self.code).await?
            == PairOutcome::WrongCode
        {
            self.register_failure(peer);
            return Ok(());
        }
        println!(
            "[A] {peer} : code correct, appareils liés en {:?}",
            started.elapsed()
        );

        // Note neuve à chaque session : chaque démonstration part du même état.
        let doc = NoteDoc::new(self.identity.yjs_client_id());
        doc.set_content(INITIAL_NOTE);
        session::exchange(&mut chan, &doc).await?;
        doc.set_content(&doc.content().replacen("- pain\n", "- pain complet\n", 1));
        session::exchange(&mut chan, &doc).await?;
        let content = session::verify_convergence(&mut chan, &doc).await?;
        println!(
            "[A] {peer} : convergence ok en {:?}, contenu {content:?}",
            started.elapsed()
        );
        Ok(())
    }

    fn register_failure(&mut self, peer: SocketAddr) {
        self.failures += 1;
        println!(
            "[A] {peer} : mauvais code, refusé ({}/{MAX_FAILURES})",
            self.failures
        );
        if self.failures >= MAX_FAILURES {
            self.code = random_pairing_code();
            self.failures = 0;
            println!("[A] trop d'échecs : nouveau code {}", self.code);
        }
    }
}

async fn accept_any(
    tcp: &TcpListener,
    tcp6: Option<&TcpListener>,
) -> std::io::Result<(TcpStream, SocketAddr)> {
    match tcp6 {
        Some(tcp6) => tokio::select! {
            accepted = tcp.accept() => accepted,
            accepted = tcp6.accept() => accepted,
        },
        None => tcp.accept().await,
    }
}

fn write_qr(
    options: &ListenOptions,
    addr: SocketAddr,
    identity: &Identity,
    code: &str,
) -> anyhow::Result<()> {
    let Some(path) = &options.qr_path else {
        return Ok(());
    };
    let uri = qr::pairing_uri(&[addr], &identity.fingerprint(), code);
    std::fs::write(path, qr::render_svg(&uri)?)?;
    println!("[A] QR code {uri:?} écrit dans {}", path.display());
    Ok(())
}

/// IP de l'interface de la route par défaut. `connect` sur une socket UDP
/// n'émet aucun paquet : il ne fait que choisir l'interface.
fn primary_lan_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("10.254.254.254:9").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    is_local(ip).then_some(ip)
}
