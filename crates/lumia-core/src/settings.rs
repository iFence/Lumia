use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSettings {
    pub language: Language,
    pub theme: ThemeMode,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: Language::English,
            theme: ThemeMode::FollowSystem,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    English,
    Chinese,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeMode {
    Light,
    Dark,
    FollowSystem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsGroup {
    General,
    Shortcuts,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_settings_default_to_english_and_follow_system() {
        let settings = AppSettings::default();

        assert_eq!(settings.language, Language::English);
        assert_eq!(settings.theme, ThemeMode::FollowSystem);
    }

    #[test]
    fn app_settings_round_trip_through_json() {
        let settings = AppSettings {
            language: Language::Chinese,
            theme: ThemeMode::Dark,
        };

        let json = serde_json::to_string(&settings).expect("serialize settings");
        let parsed: AppSettings = serde_json::from_str(&json).expect("deserialize settings");

        assert_eq!(parsed, settings);
    }
}
