//! Écoute et composition TCP, sous la politique d'adresses : une adresse hors du réseau local
//! n'est jamais composée, et une connexion qui en vient est fermée avant le handshake.

use std::io;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;

use crate::addr::AddrPolicy;
use crate::error::{Error, Result};

/// Ouvre l'écoute sur chaque adresse de `bind`, au même port. `[::]` et `0.0.0.0` vont
/// ensemble : Windows ouvre `[::]` en IPv6 seulement, d'où une seconde écoute IPv4 ; ailleurs
/// `[::]` couvre déjà l'IPv4 et la seconde écoute échoue sans conséquence. Erreur seulement si
/// aucune écoute n'a pu s'ouvrir. Rend les écoutes et le port effectif (utile avec le port 0).
pub(crate) async fn listen(bind: &[IpAddr], port: u16) -> Result<(Vec<TcpListener>, u16)> {
    let mut listeners = Vec::new();
    let mut actual = port;
    let mut failure = None;
    for ip in bind {
        match TcpListener::bind((*ip, actual)).await {
            Ok(listener) => {
                actual = listener.local_addr()?.port();
                listeners.push(listener);
            }
            Err(error) => {
                tracing::debug!(%ip, %error, "écoute impossible sur cette adresse");
                failure = Some(error);
            }
        }
    }
    if listeners.is_empty() {
        let source = failure.unwrap_or_else(|| io::Error::other("no bind address"));
        return Err(Error::Listen { port, source });
    }
    Ok((listeners, actual))
}

/// Compose la première adresse admise qui répond : toutes sont essayées en parallèle (une
/// IPv6 lien-local ou une interface virtuelle injoignable ne retarde pas les autres).
pub(crate) async fn connect(
    addrs: &[SocketAddr],
    policy: AddrPolicy,
    timeout: Duration,
) -> Result<(TcpStream, SocketAddr)> {
    let allowed: Vec<SocketAddr> = addrs
        .iter()
        .copied()
        .filter(|addr| policy.allows_socket(addr))
        .collect();
    if allowed.is_empty() {
        return Err(addrs
            .first()
            .map_or(Error::Unreachable, |addr| Error::NotLocal(addr.ip())));
    }

    let mut attempts = JoinSet::new();
    for addr in allowed {
        attempts.spawn(async move {
            let stream = tokio::time::timeout(timeout, TcpStream::connect(addr)).await;
            (addr, stream)
        });
    }
    while let Some(attempt) = attempts.join_next().await {
        if let Ok((addr, Ok(Ok(stream)))) = attempt {
            stream.set_nodelay(true)?;
            return Ok((stream, addr));
        }
    }
    Err(Error::Unreachable)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    const TIMEOUT: Duration = Duration::from_secs(2);

    #[tokio::test]
    async fn a_public_address_is_never_dialed() {
        let public: SocketAddr = "8.8.8.8:47820".parse().unwrap();
        let result = connect(&[public], AddrPolicy::LanAndLoopback, TIMEOUT).await;
        assert!(matches!(result, Err(Error::NotLocal(ip)) if ip == public.ip()));

        let loopback: SocketAddr = "127.0.0.1:47820".parse().unwrap();
        let result = connect(&[loopback], AddrPolicy::Lan, TIMEOUT).await;
        assert!(matches!(result, Err(Error::NotLocal(_))));
    }

    #[tokio::test]
    async fn the_first_reachable_address_wins() {
        let (listeners, port) = listen(&[IpAddr::V4(Ipv4Addr::LOCALHOST)], 0).await.unwrap();
        // Un port fermé à côté d'un port ouvert : seul le second aboutit
        let closed = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let closed_addr = closed.local_addr().unwrap();
        drop(closed);
        let open: SocketAddr = (Ipv4Addr::LOCALHOST, port).into();

        let (_, addr) = connect(&[closed_addr, open], AddrPolicy::LanAndLoopback, TIMEOUT)
            .await
            .unwrap();
        assert_eq!(addr, open);
        drop(listeners);
    }

    #[tokio::test]
    async fn every_bind_address_shares_the_port() {
        let bind = [IpAddr::V4(Ipv4Addr::LOCALHOST), "::1".parse().unwrap()];
        let (listeners, port) = listen(&bind, 0).await.unwrap();
        assert!(!listeners.is_empty());
        for listener in &listeners {
            assert_eq!(listener.local_addr().unwrap().port(), port);
        }
    }

    #[tokio::test]
    async fn a_taken_port_is_a_listen_error() {
        let taken = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = taken.local_addr().unwrap().port();
        let result = listen(&[IpAddr::V4(Ipv4Addr::LOCALHOST)], port).await;
        assert!(matches!(result, Err(Error::Listen { port: p, .. }) if p == port));
    }
}
