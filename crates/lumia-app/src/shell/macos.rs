use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::ptr::NonNull;

use anyhow::{bail, Context as _};
use lumia_core::ThemeMode;
use objc2_core_foundation::{CFRetained, CFString};

use super::association_formats::{extensions_for_macos_content_type, ASSOCIATION_FORMATS};
use crate::persistence::{load_file_association_preferences, save_file_association_preferences};
use crate::shell::{FileAssociationApplyResult, FileAssociationSnapshot};

const BUNDLE_ID: &str = "com.ifence.lumia";
const ROLES_VIEWER: u32 = 0x0000_0002;
const LSREGISTER: &str = "/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister";

#[link(name = "CoreServices", kind = "framework")]
extern "C" {
    fn LSCopyDefaultRoleHandlerForContentType(
        content_type: *const CFString,
        role: u32,
    ) -> *mut CFString;
    fn LSSetDefaultRoleHandlerForContentType(
        content_type: *const CFString,
        role: u32,
        handler_bundle_id: *const CFString,
    ) -> i32;
}

pub(super) fn apply_native_theme(theme: ThemeMode) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{
        NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication,
    };

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let (light_name, dark_name) = unsafe { (NSAppearanceNameAqua, NSAppearanceNameDarkAqua) };
    let appearance = match theme {
        ThemeMode::Light => NSAppearance::appearanceNamed(light_name),
        ThemeMode::Dark => NSAppearance::appearanceNamed(dark_name),
        ThemeMode::FollowSystem => None,
    };
    NSApplication::sharedApplication(mtm).setAppearance(appearance.as_deref());
}

pub(super) fn register(exe_path: &Path) -> anyhow::Result<()> {
    if let Some(bundle) = enclosing_app_bundle(exe_path) {
        register_bundle(&bundle)?;
        return Ok(());
    }

    use std::os::unix::fs::PermissionsExt;

    let home = std::env::var_os("HOME").context("HOME is not set")?;
    let contents_dir = PathBuf::from(home).join("Applications/Lumia.app/Contents");
    let macos_dir = contents_dir.join("MacOS");
    let resources_dir = contents_dir.join("Resources");
    std::fs::create_dir_all(&macos_dir)?;
    std::fs::create_dir_all(&resources_dir)?;

    let launcher_path = macos_dir.join("lumia-app");
    let launcher = format!("#!/bin/sh\nexec {} \"$@\"\n", shell_quote(exe_path));
    std::fs::write(&launcher_path, launcher)?;
    let mut permissions = std::fs::metadata(&launcher_path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&launcher_path, permissions)?;
    std::fs::write(
        contents_dir.join("Info.plist"),
        include_str!("../../resources/Info.plist"),
    )?;
    std::fs::write(
        resources_dir.join("App.icns"),
        include_bytes!("../../resources/App.icns"),
    )?;
    register_bundle(contents_dir.parent().expect("parent of Contents"))
}

pub(super) fn unregister() -> anyhow::Result<()> {
    restore_managed_defaults()?;
    let app_bundle = PathBuf::from(std::env::var_os("HOME").context("HOME is not set")?)
        .join("Applications/Lumia.app");
    if app_bundle.exists() {
        std::fs::remove_dir_all(app_bundle)?;
    }
    Ok(())
}

pub(super) fn query(_exe_path: &Path) -> anyhow::Result<FileAssociationSnapshot> {
    let preferences = load_file_association_preferences();
    let mut effective_extensions = BTreeSet::new();

    for content_type in unique_content_types() {
        if default_handler(content_type).as_deref() == Some(BUNDLE_ID) {
            effective_extensions.extend(extensions_for_macos_content_type(content_type));
        }
    }

    Ok(FileAssociationSnapshot {
        configured: preferences.configured,
        registered_extensions: all_extensions(),
        selected_extensions: preferences.selected_extensions,
        effective_extensions,
    })
}

pub(super) fn apply(
    exe_path: &Path,
    selected_extensions: &BTreeSet<String>,
) -> anyhow::Result<FileAssociationApplyResult> {
    if enclosing_app_bundle(exe_path)
        .as_ref()
        .is_some_and(|bundle| bundle.starts_with("/Volumes"))
    {
        bail!("move Lumia to Applications before making it a default application");
    }
    register(exe_path)?;
    let selected_extensions = supported_selection(selected_extensions);
    let mut preferences = load_file_association_preferences();
    let mut manual_restore_extensions = BTreeSet::new();

    for content_type in unique_content_types() {
        let extensions = extensions_for_macos_content_type(content_type);
        let desired = extensions
            .iter()
            .any(|extension| selected_extensions.contains(extension));
        let current = default_handler(content_type);

        if desired && current.as_deref() != Some(BUNDLE_ID) {
            if let Some(previous) = current.filter(|handler| !handler.is_empty()) {
                preferences
                    .previous_handlers
                    .entry(content_type.to_string())
                    .or_insert(previous);
                save_file_association_preferences(&preferences)?;
            }
            set_default_handler(content_type, BUNDLE_ID)?;
        } else if !desired && current.as_deref() == Some(BUNDLE_ID) {
            if let Some(previous) = preferences.previous_handlers.get(content_type).cloned() {
                set_default_handler(content_type, &previous)?;
                preferences.previous_handlers.remove(content_type);
            } else {
                manual_restore_extensions.extend(extensions);
            }
        } else if !desired {
            preferences.previous_handlers.remove(content_type);
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
    bail!("macOS applies file associations directly")
}

fn restore_managed_defaults() -> anyhow::Result<()> {
    let mut preferences = load_file_association_preferences();
    for (content_type, previous) in preferences.previous_handlers.clone() {
        if default_handler(&content_type).as_deref() == Some(BUNDLE_ID) {
            set_default_handler(&content_type, &previous)?;
        }
        preferences.previous_handlers.remove(&content_type);
    }
    preferences.configured = false;
    preferences.selected_extensions.clear();
    save_file_association_preferences(&preferences)?;
    Ok(())
}

fn default_handler(content_type: &str) -> Option<String> {
    let content_type = CFString::from_str(content_type);
    let raw = unsafe {
        LSCopyDefaultRoleHandlerForContentType(
            CFRetained::as_ptr(&content_type).as_ptr(),
            ROLES_VIEWER,
        )
    };
    let raw = NonNull::new(raw)?;
    let handler = unsafe { CFRetained::from_raw(raw) };
    Some(handler.to_string())
}

fn set_default_handler(content_type: &str, handler: &str) -> anyhow::Result<()> {
    let content_type = CFString::from_str(content_type);
    let handler = CFString::from_str(handler);
    let status = unsafe {
        LSSetDefaultRoleHandlerForContentType(
            CFRetained::as_ptr(&content_type).as_ptr(),
            ROLES_VIEWER,
            CFRetained::as_ptr(&handler).as_ptr(),
        )
    };
    if status != 0 {
        bail!("Launch Services rejected {content_type:?} with status {status}");
    }
    Ok(())
}

fn unique_content_types() -> BTreeSet<&'static str> {
    ASSOCIATION_FORMATS
        .iter()
        .flat_map(|format| format.macos_content_types.iter().copied())
        .collect()
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

fn enclosing_app_bundle(exe_path: &Path) -> Option<PathBuf> {
    let macos = exe_path.parent()?;
    (macos.file_name()? == "MacOS")
        .then(|| macos.parent())
        .flatten()
        .filter(|contents| contents.file_name().is_some_and(|name| name == "Contents"))
        .and_then(Path::parent)
        .filter(|bundle| {
            bundle
                .extension()
                .is_some_and(|extension| extension == "app")
        })
        .map(Path::to_path_buf)
}

fn register_bundle(bundle: &Path) -> anyhow::Result<()> {
    let output = Command::new(LSREGISTER)
        .arg("-f")
        .arg(bundle)
        .output()
        .with_context(|| format!("register {}", bundle.display()))?;
    if !output.status.success() {
        bail!(
            "Launch Services could not register Lumia: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::{enclosing_app_bundle, shell_quote};
    use std::path::{Path, PathBuf};

    #[test]
    fn identifies_only_executables_inside_app_bundles() {
        assert_eq!(
            enclosing_app_bundle(Path::new(
                "/Applications/Lumia.app/Contents/MacOS/lumia-app"
            )),
            Some(PathBuf::from("/Applications/Lumia.app"))
        );
        assert_eq!(enclosing_app_bundle(Path::new("/tmp/lumia-app")), None);
    }

    #[test]
    fn portable_launcher_quotes_apostrophes() {
        assert_eq!(
            shell_quote(Path::new("/tmp/Ada's Lumia/lumia-app")),
            r#"'/tmp/Ada'\''s Lumia/lumia-app'"#
        );
    }
}
