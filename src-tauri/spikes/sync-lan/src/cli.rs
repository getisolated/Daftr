//! Analyse minimale des arguments (pas de dépendance `clap` pour un spike).

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, bail};

const USAGE: &str = "usage :
  sync-lan-spike listen [--port P] [--code C] [--no-mdns] [--qr FICHIER.svg] [--ip-check S]
  sync-lan-spike pair <code> [--addr IP:PORT] [--fp EMPREINTE]";

/// Commande demandée.
pub enum Command {
    Listen(ListenOptions),
    Pair(PairOptions),
}

/// Options de `listen`.
pub struct ListenOptions {
    pub port: u16,
    pub code: Option<String>,
    pub mdns: bool,
    pub qr_path: Option<PathBuf>,
    /// Intervalle de détection des changements d'IP de `mdns-sd` (0 = désactivé).
    pub ip_check_secs: Option<u32>,
}

/// Options de `pair`.
pub struct PairOptions {
    pub code: String,
    pub addr: Option<SocketAddr>,
    pub fingerprint: Option<String>,
}

/// Analyse `args` (sans le nom du programme).
pub fn parse(args: Vec<String>) -> anyhow::Result<Command> {
    let mut args = args.into_iter();
    match args.next().as_deref() {
        Some("listen") => parse_listen(args).map(Command::Listen),
        Some("pair") => parse_pair(args).map(Command::Pair),
        _ => bail!("{USAGE}"),
    }
}

fn parse_listen(mut args: impl Iterator<Item = String>) -> anyhow::Result<ListenOptions> {
    let mut options = ListenOptions {
        port: 47820,
        code: None,
        mdns: true,
        qr_path: None,
        ip_check_secs: None,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--port" => options.port = value(&mut args, "--port")?.parse()?,
            "--code" => options.code = Some(value(&mut args, "--code")?),
            "--no-mdns" => options.mdns = false,
            "--ip-check" => options.ip_check_secs = Some(value(&mut args, "--ip-check")?.parse()?),
            "--qr" => options.qr_path = Some(value(&mut args, "--qr")?.into()),
            other => bail!("argument inconnu {other}\n{USAGE}"),
        }
    }
    Ok(options)
}

fn parse_pair(mut args: impl Iterator<Item = String>) -> anyhow::Result<PairOptions> {
    let code = args.next().context(USAGE)?;
    let mut options = PairOptions {
        code,
        addr: None,
        fingerprint: None,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--addr" => options.addr = Some(value(&mut args, "--addr")?.parse()?),
            "--fp" => options.fingerprint = Some(value(&mut args, "--fp")?),
            other => bail!("argument inconnu {other}\n{USAGE}"),
        }
    }
    Ok(options)
}

fn value(args: &mut impl Iterator<Item = String>, flag: &str) -> anyhow::Result<String> {
    args.next()
        .with_context(|| format!("valeur manquante pour {flag}"))
}
