//! Choix des interfaces annoncées : réseau local, ni loopback (hors tests) ni interface
//! virtuelle reconnaissable (Hyper-V, WSL, Docker, machines virtuelles, VPN).

use std::net::IpAddr;

use crate::addr::{AddrPolicy, is_link_local_v6};

/// Préfixes de noms d'interfaces virtuelles (noms Windows « conviviaux » compris).
const VIRTUAL_PREFIXES: &[&str] = &[
    "vethernet",
    "docker",
    "br-",
    "veth",
    "virbr",
    "vmnet",
    "vboxnet",
    "virtualbox",
    "vmware",
    "hyper-v",
    "wsl",
    "tun",
    "tap",
    "utun",
    "zt",
    "tailscale",
    "wg",
    "awdl",
    "llw",
    "bridge",
];

/// Le nom désigne une interface virtuelle (heuristique : les noms varient selon le système).
pub(crate) fn is_virtual(name: &str) -> bool {
    let name = name.trim().to_lowercase();
    VIRTUAL_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

/// L'adresse d'interface `ip` (interface `name`) peut être annoncée.
pub(crate) fn is_announced(name: &str, ip: IpAddr, policy: AddrPolicy) -> bool {
    policy.allows(ip) && !is_virtual(name)
}

/// Les adresses annoncées de cet appareil pour le QR code : IPv4 d'abord, sans IPv6
/// lien-local (leur index d'interface n'a de sens que localement).
pub(crate) fn lan_addresses(policy: AddrPolicy) -> Vec<IpAddr> {
    let Ok(interfaces) = if_addrs::get_if_addrs() else {
        return Vec::new();
    };
    let mut addrs: Vec<IpAddr> = interfaces
        .iter()
        .filter(|interface| is_announced(&interface.name, interface.ip(), policy))
        .map(if_addrs::Interface::ip)
        .filter(|ip| !matches!(ip, IpAddr::V6(v6) if is_link_local_v6(*v6)))
        .collect();
    addrs.sort_by_key(IpAddr::is_ipv6);
    addrs.dedup();
    addrs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_interfaces_are_recognized() {
        for name in [
            "vEthernet (WSL)",
            "vEthernet (Default Switch)",
            "docker0",
            "br-1a2b",
            "virbr0",
            "VMware Network Adapter VMnet8",
            "utun3",
            "tailscale0",
        ] {
            assert!(is_virtual(name), "{name}");
        }
        for name in [
            "Wi-Fi", "Ethernet", "en0", "eth0", "wlan0", "wlp2s0", "enp3s0",
        ] {
            assert!(!is_virtual(name), "{name}");
        }
    }

    #[test]
    fn only_local_physical_addresses_are_announced() {
        let lan: IpAddr = "192.168.1.20".parse().unwrap();
        let loopback: IpAddr = "127.0.0.1".parse().unwrap();
        let public: IpAddr = "2001:db8::1".parse().unwrap();
        assert!(is_announced("Wi-Fi", lan, AddrPolicy::Lan));
        assert!(!is_announced("vEthernet (WSL)", lan, AddrPolicy::Lan));
        assert!(!is_announced("Loopback", loopback, AddrPolicy::Lan));
        assert!(is_announced("lo", loopback, AddrPolicy::LanAndLoopback));
        assert!(!is_announced("Wi-Fi", public, AddrPolicy::Lan));
    }
}
