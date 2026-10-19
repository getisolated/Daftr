//! Notes stockées dans la base.

/// Une note telle que stockée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    /// Identifiant attribué par la base.
    pub id: i64,
    /// Titre (première ligne, extraite par le renderer).
    pub title: String,
    /// Texte markdown complet.
    pub content: String,
    /// Tags encodés en tableau JSON, tels que stockés (le renderer les décode).
    pub tags: String,
    /// Dossier de la note, `None` à la racine.
    pub folder_id: Option<i64>,
    /// Position choisie par l'utilisateur dans son dossier, `None` si jamais positionnée.
    pub sort_order: Option<i64>,
    /// Couleur `#rrggbb` de la sidebar, `None` sans couleur.
    pub color: Option<String>,
    /// Date de création (`YYYY-MM-DD HH:MM:SS`, UTC, format SQLite).
    pub created_at: String,
    /// Date de dernière modification du contenu, même format.
    pub updated_at: String,
}

/// Une note à créer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewNote {
    /// Titre.
    pub title: String,
    /// Texte markdown.
    pub content: String,
    /// Tags.
    pub tags: Vec<String>,
    /// Dossier demandé ; un dossier inexistant place la note à la racine.
    pub folder_id: Option<i64>,
}

/// Champs modifiés d'une note ; `None` laisse le champ tel quel.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct NoteChanges {
    /// Nouveau titre.
    pub title: Option<String>,
    /// Nouveau contenu.
    pub content: Option<String>,
    /// Nouveaux tags, déjà encodés en tableau JSON.
    pub tags: Option<String>,
}

impl NoteChanges {
    /// Aucun champ à modifier : la note est rendue telle quelle, sans toucher `updated_at`.
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.content.is_none() && self.tags.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_are_empty_only_without_any_field() {
        assert!(NoteChanges::default().is_empty());

        let changes = NoteChanges {
            content: Some(String::new()),
            ..NoteChanges::default()
        };
        assert!(!changes.is_empty());
    }
}
