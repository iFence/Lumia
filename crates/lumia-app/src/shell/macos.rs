use std::path::{Path, PathBuf};

use lumia_core::ThemeMode;

pub(super) fn apply_native_theme(theme: ThemeMode) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{
        NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication,
    };

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    // AppKit owns these named appearance constants for the process lifetime.
    let (light_name, dark_name) = unsafe { (NSAppearanceNameAqua, NSAppearanceNameDarkAqua) };
    let appearance = match theme {
        ThemeMode::Light => NSAppearance::appearanceNamed(light_name),
        ThemeMode::Dark => NSAppearance::appearanceNamed(dark_name),
        ThemeMode::FollowSystem => None,
    };
    NSApplication::sharedApplication(mtm).setAppearance(appearance.as_deref());
}

pub(super) fn register(exe_path: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let home = std::env::var("HOME")?;
    let contents_dir = PathBuf::from(home).join("Applications/Lumia.app/Contents");
    let macos_dir = contents_dir.join("MacOS");
    std::fs::create_dir_all(&macos_dir)?;
    std::fs::create_dir_all(contents_dir.join("Resources"))?;

    let launcher_path = macos_dir.join("lumia-app");
    let launcher = format!("#!/bin/sh\nexec '{}' \"$@\"\n", exe_path.to_string_lossy());
    std::fs::write(&launcher_path, launcher)?;
    let mut permissions = std::fs::metadata(&launcher_path)?.permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&launcher_path, permissions)?;

    std::fs::write(
        contents_dir.join("Info.plist"),
        include_str!("../../resources/Info.plist"),
    )?;

    let _ = std::process::Command::new(
        "/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister",
    )
    .arg("-f")
    .arg(contents_dir.parent().expect("parent of Contents"))
    .output();

    Ok(())
}

pub(super) fn unregister() -> anyhow::Result<()> {
    let app_bundle = PathBuf::from(std::env::var("HOME")?).join("Applications/Lumia.app");
    if app_bundle.exists() {
        std::fs::remove_dir_all(app_bundle)?;
    }
    Ok(())
}
