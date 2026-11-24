//! Type d'un appareil (portable, fixe), pour l'icône affichée à côté de son nom.

/// Type d'un appareil. Annoncé à chaque session et gardé avec l'appareil de confiance ; un
/// appareil qui ne l'annonce pas (version antérieure) reste [`DeviceKind::Unknown`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum DeviceKind {
    /// Type inconnu : détection impossible, ou appareil d'une version qui ne l'annonce pas.
    #[default]
    Unknown,
    /// Ordinateur portable (une batterie est présente).
    Laptop,
    /// Ordinateur fixe (aucune batterie).
    Desktop,
}

impl DeviceKind {
    /// Le type déduit de la présence d'une batterie : portable avec, fixe sans, inconnu
    /// quand la détection a échoué.
    ///
    /// ```
    /// use domain::sync::DeviceKind;
    ///
    /// assert_eq!(DeviceKind::from_battery(Some(true)), DeviceKind::Laptop);
    /// assert_eq!(DeviceKind::from_battery(Some(false)), DeviceKind::Desktop);
    /// assert_eq!(DeviceKind::from_battery(None), DeviceKind::Unknown);
    /// ```
    pub fn from_battery(battery: Option<bool>) -> Self {
        match battery {
            Some(true) => Self::Laptop,
            Some(false) => Self::Desktop,
            None => Self::Unknown,
        }
    }

    /// Nom stable du type (`unknown`, `laptop`, `desktop`), pour les fichiers.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Laptop => "laptop",
            Self::Desktop => "desktop",
        }
    }

    /// Le type d'un nom stable ; un nom inconnu (version ultérieure) donne `Unknown`.
    ///
    /// ```
    /// use domain::sync::DeviceKind;
    ///
    /// assert_eq!(DeviceKind::parse("laptop"), DeviceKind::Laptop);
    /// assert_eq!(DeviceKind::parse("phone"), DeviceKind::Unknown);
    /// ```
    pub fn parse(name: &str) -> Self {
        match name {
            "laptop" => Self::Laptop,
            "desktop" => Self::Desktop,
            _ => Self::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for kind in [DeviceKind::Unknown, DeviceKind::Laptop, DeviceKind::Desktop] {
            assert_eq!(DeviceKind::parse(kind.as_str()), kind);
        }
    }

    #[test]
    fn a_failed_detection_falls_back_to_unknown() {
        assert_eq!(DeviceKind::from_battery(None), DeviceKind::Unknown);
        assert_eq!(DeviceKind::default(), DeviceKind::Unknown);
    }
}
