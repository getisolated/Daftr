//! Langues de l'interface et textes du processus principal (tray, dialogues natifs).
//!
//! Les textes des fenêtres vivent côté renderer (`frontend/src/app/core/i18n`).

text_enum! {
    /// Langue de l'interface (miroir de `language.model.ts` côté renderer).
    pub enum Language {
        /// Anglais, langue de référence.
        En => "en",
        /// Français.
        Fr => "fr",
    }
}

/// Langue utilisée quand la locale de l'OS n'est pas traduite.
pub const DEFAULT_LANGUAGE: Language = Language::En;

impl Language {
    /// Langue d'une locale comme `fr-FR` ou `en_US` ; l'anglais quand elle n'est pas traduite.
    ///
    /// ```
    /// use domain::i18n::Language;
    /// assert_eq!(Language::from_locale(Some("fr-FR")), Language::Fr);
    /// assert_eq!(Language::from_locale(Some("de-DE")), Language::En);
    /// ```
    pub fn from_locale(locale: Option<&str>) -> Self {
        let lower = locale.unwrap_or_default().to_lowercase();
        let primary = lower.split(['-', '_']).next().unwrap_or_default();

        Self::parse(primary).unwrap_or(DEFAULT_LANGUAGE)
    }
}

/// Texte du processus principal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageKey {
    /// Menu du tray : ouvrir l'application.
    TrayOpenApp,
    /// Menu du tray : note rapide.
    TrayQuickNote,
    /// Menu du tray : paramètres.
    TraySettings,
    /// Menu du tray : quitter.
    TrayQuit,
    /// Titre du dialogue d'ouverture d'un dossier comme collection.
    DialogOpenFolder,
    /// Titre du dialogue d'ouverture d'un fichier markdown.
    DialogOpenFile,
    /// Filtre de fichiers markdown.
    FilterMarkdown,
    /// Filtre de pages HTML.
    FilterHtml,
    /// Filtre d'archives ZIP.
    FilterZip,
}

/// Le texte `key` dans `language`.
///
/// ```
/// use domain::i18n::{message, Language, MessageKey};
/// assert_eq!(message(Language::Fr, MessageKey::TrayQuit), "Quitter");
/// ```
pub fn message(language: Language, key: MessageKey) -> &'static str {
    match language {
        Language::En => english(key),
        Language::Fr => french(key),
    }
}

fn english(key: MessageKey) -> &'static str {
    match key {
        MessageKey::TrayOpenApp => "Open Daftr",
        MessageKey::TrayQuickNote => "Quick note",
        MessageKey::TraySettings => "Settings…",
        MessageKey::TrayQuit => "Quit",
        MessageKey::DialogOpenFolder => "Open Folder as Collection",
        MessageKey::DialogOpenFile => "Open Markdown File",
        MessageKey::FilterMarkdown => "Markdown",
        MessageKey::FilterHtml => "HTML page",
        MessageKey::FilterZip => "ZIP archive",
    }
}

fn french(key: MessageKey) -> &'static str {
    match key {
        MessageKey::TrayOpenApp => "Ouvrir Daftr",
        MessageKey::TrayQuickNote => "Note rapide",
        MessageKey::TraySettings => "Paramètres…",
        MessageKey::TrayQuit => "Quitter",
        MessageKey::DialogOpenFolder => "Ouvrir un dossier comme collection",
        MessageKey::DialogOpenFile => "Ouvrir un fichier Markdown",
        MessageKey::FilterMarkdown => "Markdown",
        MessageKey::FilterHtml => "Page HTML",
        MessageKey::FilterZip => "Archive ZIP",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follows_the_primary_subtag_of_the_locale() {
        assert_eq!(Language::from_locale(Some("fr-FR")), Language::Fr);
        assert_eq!(Language::from_locale(Some("FR_ca")), Language::Fr);
        assert_eq!(Language::from_locale(Some("fr")), Language::Fr);
        assert_eq!(Language::from_locale(Some("en-US")), Language::En);
    }

    #[test]
    fn falls_back_to_english() {
        assert_eq!(Language::from_locale(Some("de-DE")), Language::En);
        assert_eq!(Language::from_locale(Some("")), Language::En);
        assert_eq!(Language::from_locale(None), Language::En);
    }

    #[test]
    fn parses_only_known_languages() {
        assert_eq!(Language::parse("fr"), Some(Language::Fr));
        assert_eq!(Language::parse("de"), None);
    }

    #[test]
    fn translates_the_main_process_texts() {
        assert_eq!(message(Language::En, MessageKey::TrayQuit), "Quit");
        assert_eq!(message(Language::Fr, MessageKey::TrayQuit), "Quitter");
    }
}
