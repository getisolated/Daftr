//! Commandes des réglages.

use app::settings::SettingsService;
use app::settings::json::raw_from_json;
use domain::settings::AppSettings;

use crate::dto::SettingsDto;

/// Réglages courants.
pub fn get(settings: &SettingsService) -> SettingsDto {
    SettingsDto::from(&settings.current())
}

/// Réglages qui résulteraient du patch du renderer (objet JSON non fiable), sans les
/// appliquer : l'appelant vérifie d'abord le raccourci global.
pub fn candidate(settings: &SettingsService, patch: &serde_json::Value) -> AppSettings {
    settings.candidate(raw_from_json(patch))
}
