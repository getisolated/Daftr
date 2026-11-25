//! Options à choix fermé des réglages.

text_enum! {
    /// Ce qu'ouvre le raccourci global.
    pub enum ShortcutMode {
        /// L'overlay de note rapide.
        Overlay => "overlay",
        /// La fenêtre principale.
        App => "app",
    }
}

text_enum! {
    /// Taille de la barre de mise en forme.
    pub enum FormattingBarMode {
        /// Tous les boutons.
        Full => "full",
        /// Boutons essentiels.
        Compact => "compact",
    }
}

text_enum! {
    /// Style de la barre de mise en forme.
    pub enum FormattingBarStyle {
        /// Fixée au bord de l'éditeur.
        Pinned => "pinned",
        /// Flottante au-dessus du texte.
        Floating => "floating",
    }
}

text_enum! {
    /// Position de la barre de mise en forme.
    pub enum FormattingBarPosition {
        /// En haut.
        Top => "top",
        /// En bas.
        Bottom => "bottom",
        /// À gauche.
        Left => "left",
        /// À droite.
        Right => "right",
    }
}

text_enum! {
    /// Stratégie de détection des retours à la ligne dans l'aperçu markdown.
    pub enum LineBreaks {
        /// Norme markdown : une ligne vide, ou deux espaces en fin de ligne, pour un retour à la ligne.
        Markdown => "markdown",
        /// Texte brut : chaque retour à la ligne de la source en est un dans l'aperçu.
        Text => "text",
    }
}
