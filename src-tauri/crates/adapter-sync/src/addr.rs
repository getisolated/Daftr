//! Filtre d'adresses : seules les adresses du réseau local sont acceptées ou composées.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

/// Adresses admises pour la synchronisation : réseau local (RFC 1918, `169.254/16`,
/// `fc00::/7`, `fe80::/10`), et loopback seulement si la politique l'autorise (tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddrPolicy {
    /// Réseau local seulement : la politique de l'application.
    Lan,
    /// Réseau local et loopback : pour les tests sur une seule machine.
    LanAndLoopback,
}

impl AddrPolicy {
    /// L'adresse est admise (une IPv4 mappée en IPv6 est jugée comme IPv4).
    pub fn allows(self, ip: IpAddr) -> bool {
        let ip = canonical(ip);
        is_lan(ip) || (self == Self::LanAndLoopback && ip.is_loopback())
    }

    /// L'adresse de socket est admise.
    pub fn allows_socket(self, addr: &SocketAddr) -> bool {
        self.allows(addr.ip())
    }
}

/// L'IPv4 d'une adresse IPv4 mappée en IPv6 (`::ffff:a.b.c.d`, fréquente sur une écoute double
/// pile), l'adresse telle quelle sinon.
pub fn canonical(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(ip, IpAddr::V4),
        IpAddr::V4(_) => ip,
    }
}

/// L'adresse appartient à un réseau local privé ou lien-local (loopback exclu).
pub fn is_lan(ip: IpAddr) -> bool {
    match canonical(ip) {
        IpAddr::V4(v4) => is_lan_v4(v4),
        IpAddr::V6(v6) => is_lan_v6(v6),
    }
}

fn is_lan_v4(ip: Ipv4Addr) -> bool {
    ip.is_private() || ip.is_link_local()
}

fn is_lan_v6(ip: Ipv6Addr) -> bool {
    let unique_local = ip.segments()[0] & 0xfe00 == 0xfc00;
    unique_local || is_link_local_v6(ip)
}

/// IPv6 lien-local (`fe80::/10`) : joignable seulement avec l'index de son interface.
pub fn is_link_local_v6(ip: Ipv6Addr) -> bool {
    ip.segments()[0] & 0xffc0 == 0xfe80
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(text: &str) -> IpAddr {
        text.parse().unwrap()
    }

    #[test]
    fn private_and_link_local_ranges_are_allowed() {
        for text in [
            "10.0.0.1",
            "172.16.5.4",
            "172.31.255.255",
            "192.168.1.20",
            "169.254.10.10",
            "fc00::1",
            "fd12:3456::1",
            "fe80::1",
            "febf::1",
            "::ffff:192.168.1.20",
        ] {
            assert!(AddrPolicy::Lan.allows(ip(text)), "{text}");
        }
    }

    #[test]
    fn public_and_special_ranges_are_refused() {
        for text in [
            "8.8.8.8",
            "172.32.0.1",
            "192.169.0.1",
            "100.64.0.1",
            "0.0.0.0",
            "255.255.255.255",
            "224.0.0.251",
            "2001:4860:4860::8888",
            "fec0::1",
            "ff02::fb",
            "::",
            "::ffff:8.8.8.8",
        ] {
            assert!(!AddrPolicy::LanAndLoopback.allows(ip(text)), "{text}");
        }
    }

    #[test]
    fn loopback_is_allowed_only_for_tests() {
        for text in ["127.0.0.1", "::1", "::ffff:127.0.0.1"] {
            assert!(!AddrPolicy::Lan.allows(ip(text)), "{text}");
            assert!(AddrPolicy::LanAndLoopback.allows(ip(text)), "{text}");
        }
    }
}
