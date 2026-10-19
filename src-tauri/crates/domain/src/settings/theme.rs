//! Thèmes de couleurs.

text_enum! {
    /// Thème de couleurs (miroir de `theme.model.ts` côté renderer).
    pub enum ThemeId {
        /// Sombre (défaut).
        Dark => "dark",
        /// Clair.
        Light => "light",
        /// Solarized clair.
        SolarizedLight => "solarized-light",
        /// Violet clair.
        VioletLight => "violet-light",
        /// GitHub clair.
        GithubLight => "github-light",
        /// GitHub sombre atténué.
        GithubDarkDimmed => "github-dark-dimmed",
    }
}

/// Famille d'un thème.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeKind {
    /// Thème sombre.
    Dark,
    /// Thème clair.
    Light,
}

impl ThemeId {
    /// Famille claire ou sombre du thème.
    pub const fn kind(self) -> ThemeKind {
        match self {
            Self::Dark | Self::GithubDarkDimmed => ThemeKind::Dark,
            Self::Light | Self::SolarizedLight | Self::VioletLight | Self::GithubLight => {
                ThemeKind::Light
            }
        }
    }

    /// Couleur de fond `--evn-bg` du thème (`styles.scss`) : la fenêtre la montre avant que
    /// le renderer ne peigne, un thème clair ne clignote donc pas en sombre.
    pub const fn background_rgb(self) -> (u8, u8, u8) {
        match self {
            Self::Dark => (0x1c, 0x1e, 0x22),
            Self::Light | Self::GithubLight => (0xff, 0xff, 0xff),
            Self::SolarizedLight => (0xfd, 0xf6, 0xe3),
            Self::VioletLight => (0xfd, 0xfc, 0xff),
            Self::GithubDarkDimmed => (0x22, 0x27, 0x2e),
        }
    }
}

/// Thème choisi : un thème précis, ou celui qui suit le mode clair/sombre de l'OS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemePreference {
    /// Suit l'OS (avec les thèmes préférés clair et sombre).
    System,
    /// Thème fixe.
    Theme(ThemeId),
}

impl ThemePreference {
    const SYSTEM: &'static str = "system";

    /// Forme textuelle (`system` ou l'identifiant du thème).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => Self::SYSTEM,
            Self::Theme(theme) => theme.as_str(),
        }
    }

    /// Préférence correspondant à `text`, `None` sinon.
    pub fn parse(text: &str) -> Option<Self> {
        if text == Self::SYSTEM {
            return Some(Self::System);
        }
        ThemeId::parse(text).map(Self::Theme)
    }

    /// Thème effectivement affiché, selon le mode de l'OS.
    pub const fn resolve(self, os_is_dark: bool, dark: ThemeId, light: ThemeId) -> ThemeId {
        match self {
            Self::Theme(theme) => theme,
            Self::System if os_is_dark => dark,
            Self::System => light,
        }
    }
}
