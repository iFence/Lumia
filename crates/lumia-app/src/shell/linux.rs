use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context as _};

use super::association_formats::{extensions_for_linux_mime, ASSOCIATION_FORMATS};
use crate::persistence::{load_file_association_preferences, save_file_association_preferences};
use crate::shell::{FileAssociationApplyResult, FileAssociationSnapshot};

const DESKTOP_FILE_ID: &str = "lumia.desktop";
const MIME_PACKAGE: &str = "lumia-image-formats.xml";

pub(super) fn register(exe_path: &Path) -> anyhow::Result<()> {
    let applications_dir = applications_dir()?;
    let icons_dir = data_home()?.join("icons/hicolor/128x128/apps");
    let mime_packages_dir = data_home()?.join("mime/packages");
    std::fs::create_dir_all(&applications_dir)?;
    std::fs::create_dir_all(&icons_dir)?;
    std::fs::create_dir_all(&mime_packages_dir)?;

    let desktop_content = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Lumia\n\
         Comment=Fast and lightweight image viewer\n\
         Exec={} %f\n\
         Icon=lumia\n\
         Terminal=false\n\
         Categories=Graphics;Viewer;\n\
         MimeType={};\n\
         NoDisplay=false\n\
         StartupNotify=false\n",
        desktop_exec_path(exe_path),
        mime_types()
    );
    std::fs::write(applications_dir.join(DESKTOP_FILE_ID), desktop_content)?;
    std::fs::write(
        mime_packages_dir.join(MIME_PACKAGE),
        include_str!("../../resources/lumia-mime.xml"),
    )?;
    install_icon(&icons_dir)?;

    refresh_desktop_database(&applications_dir);
    refresh_mime_database(&data_home()?.join("mime"));
    Ok(())
}

pub(super) fn unregister() -> anyhow::Result<()> {
    restore_managed_defaults()?;
    let applications_dir = applications_dir()?;
    let desktop_file = applications_dir.join(DESKTOP_FILE_ID);
    let icon_file = data_home()?.join("icons/hicolor/128x128/apps/lumia.png");
    let mime_file = data_home()?.join("mime/packages").join(MIME_PACKAGE);

    remove_file_if_present(&desktop_file)?;
    remove_file_if_present(&icon_file)?;
    remove_file_if_present(&mime_file)?;
    refresh_desktop_database(&applications_dir);
    refresh_mime_database(&data_home()?.join("mime"));
    Ok(())
}

pub(super) fn query(_exe_path: &Path) -> anyhow::Result<FileAssociationSnapshot> {
    let preferences = load_file_association_preferences();
    let registered = applications_dir()?.join(DESKTOP_FILE_ID).is_file();
    let mut effective_extensions = BTreeSet::new();

    for mime in unique_mime_types() {
        if query_default(mime)?.as_deref() == Some(DESKTOP_FILE_ID) {
            effective_extensions.extend(extensions_for_linux_mime(mime));
        }
    }

    Ok(FileAssociationSnapshot {
        configured: preferences.configured,
        registered_extensions: if registered {
            all_extensions()
        } else {
            BTreeSet::new()
        },
        selected_extensions: preferences.selected_extensions,
        effective_extensions,
    })
}

pub(super) fn apply(
    exe_path: &Path,
    selected_extensions: &BTreeSet<String>,
) -> anyhow::Result<FileAssociationApplyResult> {
    register(exe_path)?;
    let selected_extensions = supported_selection(selected_extensions);
    let mut preferences = load_file_association_preferences();
    let mut manual_restore_extensions = BTreeSet::new();

    for mime in unique_mime_types() {
        let extensions = extensions_for_linux_mime(mime);
        let desired = extensions
            .iter()
            .any(|extension| selected_extensions.contains(extension));
        let current = query_default(mime)?;

        if desired && current.as_deref() != Some(DESKTOP_FILE_ID) {
            if let Some(previous) = current.filter(|handler| !handler.is_empty()) {
                preferences
                    .previous_handlers
                    .entry(mime.to_string())
                    .or_insert(previous);
                save_file_association_preferences(&preferences)?;
            }
            set_default(DESKTOP_FILE_ID, mime)?;
        } else if !desired && current.as_deref() == Some(DESKTOP_FILE_ID) {
            if let Some(previous) = preferences.previous_handlers.get(mime).cloned() {
                set_default(&previous, mime)?;
                preferences.previous_handlers.remove(mime);
            } else {
                manual_restore_extensions.extend(extensions);
            }
        } else if !desired {
            preferences.previous_handlers.remove(mime);
        }
    }

    preferences.configured = true;
    preferences.selected_extensions = selected_extensions;
    save_file_association_preferences(&preferences)?;
    let snapshot = query(exe_path)?;

    Ok(FileAssociationApplyResult {
        snapshot,
        system_confirmation_required: false,
        manual_restore_extensions,
    })
}

pub(super) fn open_default_apps_settings() -> anyhow::Result<()> {
    bail!("the Linux desktop applies file associations directly")
}

fn restore_managed_defaults() -> anyhow::Result<()> {
    let mut preferences = load_file_association_preferences();
    for (mime, previous) in preferences.previous_handlers.clone() {
        if query_default(&mime)?.as_deref() == Some(DESKTOP_FILE_ID) {
            set_default(&previous, &mime)?;
        }
        preferences.previous_handlers.remove(&mime);
    }
    preferences.configured = false;
    preferences.selected_extensions.clear();
    save_file_association_preferences(&preferences)?;
    Ok(())
}

fn query_default(mime: &str) -> anyhow::Result<Option<String>> {
    let output = Command::new("xdg-mime")
        .args(["query", "default", mime])
        .output()
        .context("run xdg-mime; install xdg-utils to manage default applications")?;
    if !output.status.success() {
        bail!(
            "xdg-mime could not query {mime}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let handler = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok((!handler.is_empty()).then_some(handler))
}

fn set_default(handler: &str, mime: &str) -> anyhow::Result<()> {
    let output = Command::new("xdg-mime")
        .args(["default", handler, mime])
        .output()
        .with_context(|| format!("set {handler} as the default for {mime}"))?;
    if !output.status.success() {
        bail!(
            "xdg-mime rejected {mime}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn unique_mime_types() -> BTreeSet<&'static str> {
    ASSOCIATION_FORMATS
        .iter()
        .flat_map(|format| format.linux_mime_types.iter().copied())
        .collect()
}

fn mime_types() -> String {
    unique_mime_types()
        .into_iter()
        .collect::<Vec<_>>()
        .join(";")
}

fn all_extensions() -> BTreeSet<String> {
    lumia_core::supported_image_extensions()
        .iter()
        .map(|extension| (*extension).to_string())
        .collect()
}

fn supported_selection(selected: &BTreeSet<String>) -> BTreeSet<String> {
    selected
        .iter()
        .filter(|extension| lumia_core::is_supported_image_extension(extension))
        .cloned()
        .collect()
}

fn data_home() -> anyhow::Result<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .context("neither XDG_DATA_HOME nor HOME is set")
}

fn applications_dir() -> anyhow::Result<PathBuf> {
    Ok(data_home()?.join("applications"))
}

fn desktop_exec_path(path: &Path) -> String {
    let escaped = path
        .to_string_lossy()
        .replace('\\', r"\\")
        .replace('"', "\\\"")
        .replace('`', "\\`")
        .replace('$', "\\$");
    format!("\"{escaped}\"")
}

fn install_icon(icons_dir: &Path) -> anyhow::Result<()> {
    let executable_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf));
    if let Some(base) = executable_dir {
        for candidate in [base.join("icon.png"), base.join("resources/icon.png")] {
            if candidate.exists() {
                std::fs::copy(candidate, icons_dir.join("lumia.png"))?;
                return Ok(());
            }
        }
    }
    std::fs::write(
        icons_dir.join("lumia.png"),
        include_bytes!("../../resources/icon.png"),
    )?;
    Ok(())
}

fn remove_file_if_present(path: &Path) -> anyhow::Result<()> {
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

fn refresh_desktop_database(directory: &Path) {
    let _ = Command::new("update-desktop-database")
        .arg(directory)
        .output();
}

fn refresh_mime_database(directory: &Path) {
    let _ = Command::new("update-mime-database").arg(directory).output();
}

#[cfg(test)]
mod tests {
    use super::{desktop_exec_path, mime_types};
    use std::path::Path;

    #[test]
    fn mime_types_are_unique_and_sorted() {
        let values = mime_types();
        let parts = values.split(';').collect::<Vec<_>>();
        assert!(parts.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(parts.contains(&"image/png"));
        assert!(parts.contains(&"image/heic"));
        assert!(parts.contains(&"image/vnd.adobe.photoshop"));
    }

    #[test]
    fn desktop_exec_quotes_shell_sensitive_paths() {
        assert_eq!(
            desktop_exec_path(Path::new("/tmp/Lumia $Build/app")),
            r#""/tmp/Lumia \$Build/app""#
        );
    }
}
