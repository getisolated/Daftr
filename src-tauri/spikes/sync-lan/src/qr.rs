//! Contenu et rendu SVG du QR code d'appairage.

use std::net::SocketAddr;

use qrcode::QrCode;
use qrcode::render::svg;

/// `daftr-pair:1:<adresses IP:port>:<empreinte>:<code>` (§6). Plusieurs adresses
/// sont séparées par des virgules ; une IPv6 est entre crochets (`[fe80::1]:47820`),
/// donc l'analyse doit partir des deux extrémités.
pub fn pairing_uri(addrs: &[SocketAddr], fingerprint: &str, code: &str) -> String {
    let addrs: Vec<String> = addrs.iter().map(SocketAddr::to_string).collect();
    format!("daftr-pair:1:{}:{fingerprint}:{code}", addrs.join(","))
}

/// Analyse un contenu de QR code : (adresses, empreinte, code).
pub fn parse_pairing_uri(uri: &str) -> Option<(Vec<SocketAddr>, String, String)> {
    let rest = uri.strip_prefix("daftr-pair:1:")?;
    let mut tail = rest.rsplitn(3, ':');
    let code = tail.next()?.to_owned();
    let fingerprint = tail.next()?.to_owned();
    let addrs = tail
        .next()?
        .split(',')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    Some((addrs, fingerprint, code))
}

/// Rend le QR code en SVG (aucune dépendance image).
pub fn render_svg(content: &str) -> anyhow::Result<String> {
    let code = QrCode::new(content.as_bytes())?;
    Ok(code.render::<svg::Color>().min_dimensions(240, 240).build())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_round_trips_with_ipv4_and_ipv6() {
        let addrs: Vec<SocketAddr> = vec![
            "192.168.1.20:47820".parse().unwrap(),
            "[fe80::1]:47820".parse().unwrap(),
        ];
        let uri = pairing_uri(&addrs, "ab12", "012345");
        assert_eq!(
            uri,
            "daftr-pair:1:192.168.1.20:47820,[fe80::1]:47820:ab12:012345"
        );
        assert_eq!(
            parse_pairing_uri(&uri),
            Some((addrs, "ab12".into(), "012345".into()))
        );
    }

    #[test]
    fn svg_is_rendered() {
        let svg = render_svg("daftr-pair:1:192.168.1.20:47820:ab12:012345").unwrap();
        assert!(svg.contains("<svg"));
    }
}
