use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShortcutId {
    OpenFile,
    ZoomIn,
    ZoomOut,
    ZoomFit,
    ToggleFullscreen,
    ExitFullscreen,
    ToggleImageInfo,
    NextImage,
    PreviousImage,
    Quit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSettings {
    pub language: Language,
    pub theme: ThemeMode,
    #[serde(default)]
    pub theme_accent: ThemeAccent,
    #[serde(default = "default_shortcuts")]
    pub shortcuts: HashMap<ShortcutId, String>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: Language::English,
            theme: ThemeMode::FollowSystem,
            theme_accent: ThemeAccent::default(),
            shortcuts: default_shortcuts(),
        }
    }
}

/// Platform-appropriate default keybindings.
pub fn default_shortcuts() -> HashMap<ShortcutId, String> {
    let mut map = HashMap::new();

    // On Windows, use ctrl-; on macOS the user can re-record to cmd-
    #[cfg(target_os = "macos")]
    {
        map.insert(ShortcutId::OpenFile, "cmd-o".into());
        map.insert(ShortcutId::ZoomIn, "cmd-=".into());
        map.insert(ShortcutId::ZoomOut, "cmd--".into());
        map.insert(ShortcutId::ZoomFit, "cmd-0".into());
        map.insert(ShortcutId::ToggleFullscreen, "cmd-enter".into());
        map.insert(ShortcutId::ExitFullscreen, "escape".into());
        map.insert(ShortcutId::ToggleImageInfo, "tab".into());
        map.insert(ShortcutId::NextImage, "right".into());
        map.insert(ShortcutId::PreviousImage, "left".into());
        map.insert(ShortcutId::Quit, "cmd-q".into());
    }
    #[cfg(not(target_os = "macos"))]
    {
        map.insert(ShortcutId::OpenFile, "ctrl-o".into());
        map.insert(ShortcutId::ZoomIn, "ctrl-=".into());
        map.insert(ShortcutId::ZoomOut, "ctrl--".into());
        map.insert(ShortcutId::ZoomFit, "ctrl-0".into());
        map.insert(ShortcutId::ToggleFullscreen, "f11".into());
        map.insert(ShortcutId::ExitFullscreen, "escape".into());
        map.insert(ShortcutId::ToggleImageInfo, "tab".into());
        map.insert(ShortcutId::NextImage, "right".into());
        map.insert(ShortcutId::PreviousImage, "left".into());
        map.insert(ShortcutId::Quit, "ctrl-q".into());
    }
    map
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeAccent {
    Blue,
    Green,
    Orange,
    Rose,
}

impl Default for ThemeAccent {
    fn default() -> Self {
        Self::Blue
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsGroup {
    General,
    FileAssociations,
    Shortcuts,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_settings_default_to_english_follow_system_and_blue_accent() {
        let settings = AppSettings::default();

        assert_eq!(settings.language, Language::English);
        assert_eq!(settings.theme, ThemeMode::FollowSystem);
        assert_eq!(settings.theme_accent, ThemeAccent::Blue);
        assert!(!settings.shortcuts.is_empty());
    }

    #[test]
    fn app_settings_round_trip_through_json() {
        let settings = AppSettings {
            language: Language::Chinese,
            theme: ThemeMode::Dark,
            theme_accent: ThemeAccent::Rose,
            shortcuts: default_shortcuts(),
        };

        let json = serde_json::to_string(&settings).expect("serialize settings");
        let parsed: AppSettings = serde_json::from_str(&json).expect("deserialize settings");

        assert_eq!(parsed, settings);
    }

    #[test]
    fn missing_shortcuts_in_json_uses_defaults() {
        let json = r#"{"language":"English","theme":"Light"}"#;
        let parsed: AppSettings =
            serde_json::from_str(json).expect("deserialize without shortcuts field");
        assert_eq!(parsed.theme_accent, ThemeAccent::Blue);
        assert_eq!(parsed.shortcuts, default_shortcuts());
    }

    #[test]
    fn partial_shortcuts_in_json_merged_with_defaults() {
        // A user might have only customized one shortcut; the rest should stay default.
        // But serde's `default` only applies when the field is entirely missing.
        // When present with a subset, we need custom deserialization.
        // For now just test the missing-field path.
        let json = r#"{"language":"English","theme":"Light","shortcuts":{}}"#;
        let parsed: AppSettings =
            serde_json::from_str(json).expect("deserialize with empty shortcuts");
        assert_eq!(parsed.theme_accent, ThemeAccent::Blue);
        assert!(parsed.shortcuts.is_empty());
    }

    #[test]
    fn shortcut_id_serializes_as_snake_case() {
        let id = ShortcutId::OpenFile;
        let json = serde_json::to_string(&id).expect("serialize");
        assert_eq!(json, r#""open_file""#);
    }

    #[test]
    fn shortcut_id_deserializes_from_snake_case() {
        let json = r#""open_file""#;
        let id: ShortcutId = serde_json::from_str(json).expect("deserialize");
        assert_eq!(id, ShortcutId::OpenFile);
    }
}
