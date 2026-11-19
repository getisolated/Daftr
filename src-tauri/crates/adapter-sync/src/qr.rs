//! Contenu et rendu du QR code d'appairage, format figé pour tous les clients :
//!
//! `daftr-pair:1:<adresses>:<empreinte>:<code>`
//!
//! - `<adresses>` : une ou plusieurs `IP:port` séparées par des virgules, IPv6 entre crochets
//!   (`192.168.1.20:47820,[fd00::5]:47820`) ; pas d'IPv6 lien-local (l'index d'interface
//!   n'a de sens que sur l'appareil qui l'annonce) ;
//! - `<empreinte>` : 32 chiffres hexadécimaux, voir [`Fingerprint`] ;
//! - `<code>` : les 6 chiffres du code.
//!
//! Les adresses contiennent des `:`, donc l'analyse découpe le code puis l'empreinte depuis la
//! fin. Espaces autour ignorés, chiffres hexadécimaux majuscules admis.

use std::fmt;
use std::net::SocketAddr;

use domain::sync::PairingCode;
use qrcode::QrCode;
use qrcode::render::svg;

use crate::error::{Error, Result};
use crate::keys::Fingerprint;

const PREFIX: &str = "daftr-pair:";
const VERSION: &str = "1";
/// Adresses au plus dans le QR code (au-delà, il devient trop dense pour un scan fiable).
pub const MAX_QR_ADDRS: usize = 4;
/// Taille minimale du rendu SVG, en pixels.
const SVG_MIN_SIZE: u32 = 240;

/// Contenu d'un QR code d'appairage.
#[derive(Clone, PartialEq, Eq)]
pub struct PairingUri {
    /// Adresses de l'appareil qui affiche le QR code.
    pub addrs: Vec<SocketAddr>,
    /// Empreinte de sa clé publique.
    pub fingerprint: Fingerprint,
    /// Code d'appairage.
    pub code: PairingCode,
}

impl PairingUri {
    /// Analyse le contenu d'un QR code scanné.
    pub fn parse(text: &str) -> Result<Self> {
        let rest = text
            .trim()
            .strip_prefix(PREFIX)
            .and_then(|rest| rest.strip_prefix(VERSION))
            .and_then(|rest| rest.strip_prefix(':'))
            .ok_or(Error::InvalidQr)?;
        let mut fields = rest.rsplitn(3, ':');
        let code = fields.next().and_then(PairingCode::parse);
        let fingerprint = fields.next().and_then(Fingerprint::parse);
        let (Some(code), Some(fingerprint), Some(addrs)) = (code, fingerprint, fields.next())
        else {
            return Err(Error::InvalidQr);
        };
        Ok(Self {
            addrs: parse_addrs(addrs)?,
            fingerprint,
            code,
        })
    }

    /// Rend le QR code en SVG (aucune dépendance image).
    pub fn to_svg(&self) -> Result<String> {
        let code = QrCode::new(self.to_string().as_bytes()).map_err(|_| Error::InvalidQr)?;
        Ok(code
            .render::<svg::Color<'_>>()
            .min_dimensions(SVG_MIN_SIZE, SVG_MIN_SIZE)
            .build())
    }
}

fn parse_addrs(text: &str) -> Result<Vec<SocketAddr>> {
    let addrs = text
        .split(',')
        .map(|addr| addr.trim().parse::<SocketAddr>())
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| Error::InvalidQr)?;
    if addrs.is_empty() || addrs.len() > MAX_QR_ADDRS {
        return Err(Error::InvalidQr);
    }
    Ok(addrs)
}

impl fmt::Display for PairingUri {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{PREFIX}{VERSION}:")?;
        for (index, addr) in self.addrs.iter().enumerate() {
            if index > 0 {
                formatter.write_str(",")?;
            }
            write!(formatter, "{addr}")?;
        }
        write!(formatter, ":{}:{}", self.fingerprint, self.code.as_str())
    }
}

/// Le code n'apparaît jamais dans les traces.
impl fmt::Debug for PairingUri {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PairingUri")
            .field("addrs", &self.addrs)
            .field("fingerprint", &self.fingerprint)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::sync::DeviceKey;

    fn fingerprint() -> Fingerprint {
        Fingerprint::of(&DeviceKey::from_bytes([1; 32]))
    }

    fn uri(addrs: &[&str]) -> PairingUri {
        PairingUri {
            addrs: addrs.iter().map(|addr| addr.parse().unwrap()).collect(),
            fingerprint: fingerprint(),
            code: PairingCode::parse("012345").unwrap(),
        }
    }

    #[test]
    fn the_format_is_frozen() {
        let uri = uri(&["192.168.1.20:47820", "[fd00::5]:47820"]);
        let expected = format!(
            "daftr-pair:1:192.168.1.20:47820,[fd00::5]:47820:{}:012345",
            fingerprint()
        );
        assert_eq!(uri.to_string(), expected);
    }

    #[test]
    fn uris_round_trip() {
        for addrs in [
            &["10.0.0.2:1"][..],
            &["[fe80::1%7]:47820"],
            &["192.168.1.20:47820", "[fd00::5]:47820", "10.1.2.3:9"],
        ] {
            let uri = uri(addrs);
            assert_eq!(PairingUri::parse(&uri.to_string()).unwrap(), uri);
        }
    }

    #[test]
    fn parsing_is_lenient_on_spacing_and_case() {
        let text = format!(
            "  daftr-pair:1:10.0.0.2:47820 , 10.0.0.3:47820:{}:012345\n",
            fingerprint().to_string().to_uppercase()
        );
        let parsed = PairingUri::parse(&text).unwrap();
        assert_eq!(parsed.addrs.len(), 2);
        assert_eq!(parsed.fingerprint, fingerprint());
    }

    #[test]
    fn invalid_contents_are_refused() {
        let fp = fingerprint();
        let too_many: Vec<String> = (2..7).map(|n| format!("10.0.0.{n}:1")).collect();
        for text in [
            String::new(),
            format!("daftr-pair:2:10.0.0.2:1:{fp}:012345"),
            format!("other:1:10.0.0.2:1:{fp}:012345"),
            format!("daftr-pair:1::{fp}:012345"),
            format!("daftr-pair:1:10.0.0.2:{fp}:012345"),
            format!("daftr-pair:1:10.0.0.2:1:{fp}:12345"),
            "daftr-pair:1:10.0.0.2:1:abcd:012345".to_owned(),
            format!("daftr-pair:1:{}:{fp}:012345", too_many.join(",")),
        ] {
            assert!(PairingUri::parse(&text).is_err(), "{text}");
        }
    }

    #[test]
    fn the_code_stays_out_of_debug_output_and_the_svg_renders() {
        let uri = uri(&["10.0.0.2:47820"]);
        assert!(!format!("{uri:?}").contains("012345"));
        assert!(uri.to_svg().unwrap().contains("<svg"));
    }
}
