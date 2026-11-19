//! Filtre d'adresses : seules les IP locales sont acceptées ou composées.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Vrai si l'adresse appartient à un réseau local : RFC 1918, `169.254/16`,
/// `fc00::/7`, `fe80::/10` ou loopback (tests). Les IPv4 mappées en IPv6
/// (`::ffff:a.b.c.d`, fréquentes sur une socket double pile) sont ramenées en IPv4.
pub fn is_local(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_local_v4(v4),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => is_local_v4(v4),
            None => is_local_v6(v6),
        },
    }
}

fn is_local_v4(ip: Ipv4Addr) -> bool {
    ip.is_private() || ip.is_link_local() || ip.is_loopback()
}

fn is_local_v6(ip: Ipv6Addr) -> bool {
    let first = ip.segments()[0];
    let unique_local = first & 0xfe00 == 0xfc00;
    let link_local = first & 0xffc0 == 0xfe80;
    ip.is_loopback() || unique_local || link_local
}

#[cfg(test)]
mod tests {
    use super::is_local;
    use std::net::IpAddr;

    fn local(s: &str) -> bool {
        is_local(s.parse::<IpAddr>().unwrap())
    }

    #[test]
    fn accepts_private_ranges() {
        for ip in [
            "10.0.0.1",
            "172.16.5.4",
            "172.31.255.255",
            "192.168.1.20",
            "169.254.10.10",
            "127.0.0.1",
            "::1",
            "fc00::1",
            "fd12:3456::1",
            "fe80::1",
            "febf::1",
            "::ffff:192.168.1.20",
        ] {
            assert!(local(ip), "{ip} devrait être acceptée");
        }
    }

    #[test]
    fn rejects_public_and_special_ranges() {
        for ip in [
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
            assert!(!local(ip), "{ip} devrait être refusée");
        }
    }
}
