use lumia_core::AppSettings;
use std::{env, fs, io, path::PathBuf};

#[cfg(target_os = "windows")]
use lumia_core::Language;

use crate::shell::FileAssociationPreferences;
use crate::APP_TITLE;

pub(crate) fn load_settings() -> AppSettings {
    let saved = settings_path()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|json| serde_json::from_str(&json).ok());
    saved.unwrap_or_else(installed_default_settings)
}

pub(crate) fn save_settings(settings: &AppSettings) -> io::Result<()> {
    let Some(path) = settings_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(io::Error::other)?;
    fs::write(path, json)
}

pub(crate) fn load_file_association_preferences() -> FileAssociationPreferences {
    associations_path()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

pub(crate) fn save_file_association_preferences(
    preferences: &FileAssociationPreferences,
) -> io::Result<()> {
    let Some(path) = associations_path() else {
        return Ok(());
    };
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    fs::create_dir_all(parent)?;
    let temporary = path.with_extension("json.tmp");
    let json = serde_json::to_string_pretty(preferences).map_err(io::Error::other)?;
    fs::write(&temporary, json)?;
    fs::rename(temporary, path)
}

fn settings_path() -> Option<PathBuf> {
    platform_config_dir().map(|dir| dir.join("settings.json"))
}

fn associations_path() -> Option<PathBuf> {
    platform_config_dir().map(|dir| dir.join("associations.json"))
}

fn platform_config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|path| path.join(APP_TITLE))
    }

    #[cfg(target_os = "macos")]
    {
        env::var_os("HOME").map(PathBuf::from).map(|path| {
            path.join("Library")
                .join("Application Support")
                .join(APP_TITLE)
        })
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .map(|path| path.join("lumia"))
    }
}

fn installed_default_settings() -> AppSettings {
    #[cfg(target_os = "windows")]
    {
        let mut settings = AppSettings::default();
        if let Some(language) = installed_language() {
            settings.language = language;
        }
        settings
    }
    #[cfg(not(target_os = "windows"))]
    {
        AppSettings::default()
    }
}

#[cfg(target_os = "windows")]
fn installed_language() -> Option<Language> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let installer = hkcu.open_subkey(r"Software\Lumia\Installer").ok()?;
    match installer
        .get_value::<String, _>("InstallLanguage")
        .ok()?
        .as_str()
    {
        "zh-CN" => Some(Language::Chinese),
        "en-US" => Some(Language::English),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_defaults_change_only_language() {
        let defaults = installed_default_settings();
        let baseline = AppSettings::default();
        assert_eq!(defaults.theme_accent, baseline.theme_accent);
        assert_eq!(defaults.shortcuts, baseline.shortcuts);
    }
}
