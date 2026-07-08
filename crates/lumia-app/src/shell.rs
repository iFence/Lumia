#[allow(unused_imports)]
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Public cross-platform API
// ---------------------------------------------------------------------------

/// Register Lumia in the operating-system context menu so that right-clicking
/// an image file offers "Open with Lumia".
pub(crate) fn register_context_menu() -> anyhow::Result<()> {
    let exe_path = std::env::current_exe()?;
    register_platform(&exe_path)
}

/// Remove Lumia from the operating-system context menu.
pub(crate) fn unregister_context_menu() -> anyhow::Result<()> {
    unregister_platform()
}

// ---------------------------------------------------------------------------
// Per-platform dispatch
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn register_platform(exe_path: &Path) -> anyhow::Result<()> {
    register_windows(exe_path)
}

#[cfg(target_os = "windows")]
fn unregister_platform() -> anyhow::Result<()> {
    unregister_windows()
}

#[cfg(target_os = "macos")]
fn register_platform(exe_path: &Path) -> anyhow::Result<()> {
    register_macos(exe_path)
}

#[cfg(target_os = "macos")]
fn unregister_platform() -> anyhow::Result<()> {
    unregister_macos()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn register_platform(exe_path: &Path) -> anyhow::Result<()> {
    register_linux(exe_path)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn unregister_platform() -> anyhow::Result<()> {
    unregister_linux()
}

// ===========================================================================
// Windows — HKCU registry entries
// ===========================================================================

#[cfg(target_os = "windows")]
fn register_windows(exe_path: &Path) -> anyhow::Result<()> {
    use winreg::enums::*;
    use winreg::RegKey;

    let exe_str = exe_path.to_string_lossy().to_string();
    let command = format!("\"{}\" \"%1\"", exe_str);

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let classes = hkcu.create_subkey("Software\\Classes")?.0;

    // 1. ProgId — Lumia.Image
    {
        let (progid_key, _) = classes.create_subkey("Lumia.Image\\shell\\open\\command")?;
        progid_key.set_value("", &command)?;
    }

    // 2. SystemFileAssociations — appears for all generic "image" types
    {
        let (sys_key, _) =
            classes.create_subkey("SystemFileAssociations\\image\\shell\\Lumia\\command")?;
        sys_key.set_value("", &command)?;
    }

    // 3. Per-extension OpenWithProgids
    for ext in lumia_core::supported_image_extensions() {
        let subkey_path = format!(".{}\\OpenWithProgids", ext);
        if let Ok((ext_key, _)) = classes.create_subkey(&subkey_path) {
            let _ = ext_key.set_value("Lumia.Image", &"");
        }
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn unregister_windows() -> anyhow::Result<()> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let classes = hkcu.open_subkey_with_flags("Software\\Classes", KEY_ALL_ACCESS)?;

    // Delete the ProgId key tree
    let _ = classes.delete_subkey_all("Lumia.Image");

    // Delete the SystemFileAssociations entry
    let _ = classes.delete_subkey_all("SystemFileAssociations\\image\\shell\\Lumia");

    // Remove per-extension entries
    for ext in lumia_core::supported_image_extensions() {
        let subkey_path = format!(".{}\\OpenWithProgids", ext);
        if let Ok(ext_key) = classes.open_subkey_with_flags(&subkey_path, KEY_ALL_ACCESS) {
            let _ = ext_key.delete_value("Lumia.Image");
        }
    }

    Ok(())
}

// ===========================================================================
// macOS — wrapper .app bundle under ~/Applications/
// ===========================================================================

#[cfg(target_os = "macos")]
fn register_macos(exe_path: &Path) -> anyhow::Result<()> {
    use std::io::Write;

    let home = std::env::var("HOME")?;
    let app_dir = PathBuf::from(&home).join("Applications/Lumia.app/Contents");
    let macos_dir = app_dir.join("MacOS");
    let resources_dir = app_dir.join("Resources");
    std::fs::create_dir_all(&macos_dir)?;
    std::fs::create_dir_all(&resources_dir)?;

    // Write a tiny launcher script that execs the real binary
    let launcher_path = macos_dir.join("lumia-app");
    let launcher_script = format!(
        "#!/bin/sh\n\
         exec '{}' \"$@\"\n",
        exe_path.to_string_lossy()
    );
    std::fs::write(&launcher_path, launcher_script)?;

    // Make launcher executable
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(&launcher_path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&launcher_path, perms)?;

    // Write Info.plist
    let plist_path = app_dir.join("Info.plist");
    let plist = build_macos_info_plist();
    std::fs::write(&plist_path, plist)?;

    // Register with Launch Services so Finder picks it up immediately
    let _ = std::process::Command::new("/System/Library/Frameworks/CoreServices.framework/Versions/A/Frameworks/LaunchServices.framework/Versions/A/Support/lsregister")
        .arg("-f")
        .arg(app_dir.parent().expect("parent of Contents"))
        .output();

    Ok(())
}

#[cfg(target_os = "macos")]
fn unregister_macos() -> anyhow::Result<()> {
    let home = std::env::var("HOME")?;
    let app_bundle = PathBuf::from(&home).join("Applications/Lumia.app");

    if app_bundle.exists() {
        std::fs::remove_dir_all(&app_bundle)?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn build_macos_info_plist() -> String {
    // Declare support for standard UTIs + custom UTIs for niche formats.
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>Lumia</string>
    <key>CFBundleDisplayName</key>
    <string>Lumia</string>
    <key>CFBundleIdentifier</key>
    <string>com.ifence.lumia</string>
    <key>CFBundleVersion</key>
    <string>0.1.0</string>
    <key>CFBundleShortVersionString</key>
    <string>0.1.0</string>
    <key>CFBundleExecutable</key>
    <string>lumia-app</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>LSMultipleInstancesProhibited</key>
    <false/>
    <key>CFBundleDocumentTypes</key>
    <array>
        <dict>
            <key>CFBundleTypeName</key><string>AVIF image</string>
            <key>CFBundleTypeRole</key><string>Viewer</string>
            <key>LSHandlerRank</key><string>Alternate</string>
            <key>LSItemContentTypes</key><array><string>public.avif</string></array>
        </dict>
        <dict>
            <key>CFBundleTypeName</key><string>BMP image</string>
            <key>CFBundleTypeRole</key><string>Viewer</string>
            <key>LSHandlerRank</key><string>Alternate</string>
            <key>LSItemContentTypes</key><array><string>com.microsoft.bmp</string></array>
        </dict>
        <dict>
            <key>CFBundleTypeName</key><string>DDS image</string>
            <key>CFBundleTypeRole</key><string>Viewer</string>
            <key>LSHandlerRank</key><string>Alternate</string>
            <key>LSItemContentTypes</key><array><string>com.ifence.lumia.dds</string></array>
        </dict>
        <dict>
            <key>CFBundleTypeName</key><string>EXR image</string>
            <key>CFBundleTypeRole</key><string>Viewer</string>
            <key>LSHandlerRank</key><string>Alternate</string>
            <key>LSItemContentTypes</key><array><string>com.ifence.lumia.exr</string></array>
        </dict>
        <dict>
            <key>CFBundleTypeName</key><string>Farbfeld image</string>
            <key>CFBundleTypeRole</key><string>Viewer</string>
            <key>LSHandlerRank</key><string>Alternate</string>
            <key>LSItemContentTypes</key><array><string>com.ifence.lumia.farbfeld</string></array>
        </dict>
        <dict>
            <key>CFBundleTypeName</key><string>GIF image</string>
            <key>CFBundleTypeRole</key><string>Viewer</string>
            <key>LSHandlerRank</key><string>Alternate</string>
            <key>LSItemContentTypes</key><array><string>public.gif</string></array>
        </dict>
        <dict>
            <key>CFBundleTypeName</key><string>HDR image</string>
            <key>CFBundleTypeRole</key><string>Viewer</string>
            <key>LSHandlerRank</key><string>Alternate</string>
            <key>LSItemContentTypes</key><array><string>com.ifence.lumia.hdr</string></array>
        </dict>
        <dict>
            <key>CFBundleTypeName</key><string>HEIC image</string>
            <key>CFBundleTypeRole</key><string>Viewer</string>
            <key>LSHandlerRank</key><string>Alternate</string>
            <key>LSItemContentTypes</key><array><string>public.heic</string></array>
        </dict>
        <dict>
            <key>CFBundleTypeName</key><string>HEIF image</string>
            <key>CFBundleTypeRole</key><string>Viewer</string>
            <key>LSHandlerRank</key><string>Alternate</string>
            <key>LSItemContentTypes</key><array><string>public.heif</string></array>
        </dict>
        <dict>
            <key>CFBundleTypeName</key><string>ICO image</string>
            <key>CFBundleTypeRole</key><string>Viewer</string>
            <key>LSHandlerRank</key><string>Alternate</string>
            <key>LSItemContentTypes</key><array><string>com.microsoft.ico</string></array>
        </dict>
        <dict>
            <key>CFBundleTypeName</key><string>JPEG image</string>
            <key>CFBundleTypeRole</key><string>Viewer</string>
            <key>LSHandlerRank</key><string>Alternate</string>
            <key>LSItemContentTypes</key><array><string>public.jpeg</string></array>
        </dict>
        <dict>
            <key>CFBundleTypeName</key><string>Netpbm image</string>
            <key>CFBundleTypeRole</key><string>Viewer</string>
            <key>LSHandlerRank</key><string>Alternate</string>
            <key>LSItemContentTypes</key><array><string>com.ifence.lumia.netpbm</string></array>
        </dict>
        <dict>
            <key>CFBundleTypeName</key><string>PNG image</string>
            <key>CFBundleTypeRole</key><string>Viewer</string>
            <key>LSHandlerRank</key><string>Alternate</string>
            <key>LSItemContentTypes</key><array><string>public.png</string></array>
        </dict>
        <dict>
            <key>CFBundleTypeName</key><string>QOI image</string>
            <key>CFBundleTypeRole</key><string>Viewer</string>
            <key>LSHandlerRank</key><string>Alternate</string>
            <key>LSItemContentTypes</key><array><string>com.ifence.lumia.qoi</string></array>
        </dict>
        <dict>
            <key>CFBundleTypeName</key><string>SVG image</string>
            <key>CFBundleTypeRole</key><string>Viewer</string>
            <key>LSHandlerRank</key><string>Alternate</string>
            <key>LSItemContentTypes</key><array><string>public.svg-image</string></array>
        </dict>
        <dict>
            <key>CFBundleTypeName</key><string>TGA image</string>
            <key>CFBundleTypeRole</key><string>Viewer</string>
            <key>LSHandlerRank</key><string>Alternate</string>
            <key>LSItemContentTypes</key><array><string>com.truevision.tga-image</string></array>
        </dict>
        <dict>
            <key>CFBundleTypeName</key><string>TIFF image</string>
            <key>CFBundleTypeRole</key><string>Viewer</string>
            <key>LSHandlerRank</key><string>Alternate</string>
            <key>LSItemContentTypes</key><array><string>public.tiff</string></array>
        </dict>
        <dict>
            <key>CFBundleTypeName</key><string>WebP image</string>
            <key>CFBundleTypeRole</key><string>Viewer</string>
            <key>LSHandlerRank</key><string>Alternate</string>
            <key>LSItemContentTypes</key><array><string>org.webmproject.webp</string></array>
        </dict>
    </array>
    <key>UTExportedTypeDeclarations</key>
    <array>
        <dict>
            <key>UTTypeIdentifier</key><string>com.ifence.lumia.dds</string>
            <key>UTTypeTagSpecification</key>
            <dict><key>public.filename-extension</key><array><string>dds</string></array></dict>
        </dict>
        <dict>
            <key>UTTypeIdentifier</key><string>com.ifence.lumia.exr</string>
            <key>UTTypeTagSpecification</key>
            <dict><key>public.filename-extension</key><array><string>exr</string></array></dict>
        </dict>
        <dict>
            <key>UTTypeIdentifier</key><string>com.ifence.lumia.farbfeld</string>
            <key>UTTypeTagSpecification</key>
            <dict><key>public.filename-extension</key><array><string>ff</string><string>farbfeld</string></array></dict>
        </dict>
        <dict>
            <key>UTTypeIdentifier</key><string>com.ifence.lumia.hdr</string>
            <key>UTTypeTagSpecification</key>
            <dict><key>public.filename-extension</key><array><string>hdr</string></array></dict>
        </dict>
        <dict>
            <key>UTTypeIdentifier</key><string>com.ifence.lumia.netpbm</string>
            <key>UTTypeTagSpecification</key>
            <dict><key>public.filename-extension</key><array><string>pbm</string><string>pam</string><string>ppm</string><string>pgm</string></array></dict>
        </dict>
        <dict>
            <key>UTTypeIdentifier</key><string>com.ifence.lumia.qoi</string>
            <key>UTTypeTagSpecification</key>
            <dict><key>public.filename-extension</key><array><string>qoi</string></array></dict>
        </dict>
    </array>
</dict>
</plist>"#
    )
}

// ===========================================================================
// Linux — .desktop file + MIME registration
// ===========================================================================

#[cfg(all(unix, not(target_os = "macos")))]
fn register_linux(exe_path: &Path) -> anyhow::Result<()> {
    let home = std::env::var("HOME")?;
    let apps_dir = PathBuf::from(&home).join(".local/share/applications");
    let icons_dir = PathBuf::from(&home).join(".local/share/icons/hicolor/128x128/apps");
    std::fs::create_dir_all(&apps_dir)?;
    std::fs::create_dir_all(&icons_dir)?;

    // Write .desktop file
    let desktop_path = apps_dir.join("lumia.desktop");
    let exec = exe_path.to_string_lossy().to_string();
    let mime_types = linux_mime_types();
    let desktop_content = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Lumia\n\
         Comment=Fast and lightweight image viewer\n\
         Exec={} %f\n\
         Icon=lumia\n\
         Terminal=false\n\
         Categories=Graphics;Viewer;\n\
         MimeType={}\n\
         NoDisplay=false\n\
         StartupNotify=false\n",
        exec, mime_types
    );
    std::fs::write(&desktop_path, desktop_content)?;

    // Install icon from the executable's sibling directory or embedded resource
    install_linux_icon(&icons_dir)?;

    // Try to update the desktop database (non-fatal if missing)
    let _ = std::process::Command::new("update-desktop-database")
        .arg(apps_dir.to_string_lossy().as_ref())
        .output();

    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn unregister_linux() -> anyhow::Result<()> {
    let home = std::env::var("HOME")?;
    let desktop_file = PathBuf::from(&home).join(".local/share/applications/lumia.desktop");
    let icon_file = PathBuf::from(&home).join(".local/share/icons/hicolor/128x128/apps/lumia.png");

    if desktop_file.exists() {
        std::fs::remove_file(&desktop_file)?;
    }
    if icon_file.exists() {
        std::fs::remove_file(&icon_file)?;
    }

    // Try to refresh the desktop database
    let apps_dir = PathBuf::from(&home).join(".local/share/applications");
    let _ = std::process::Command::new("update-desktop-database")
        .arg(apps_dir.to_string_lossy().as_ref())
        .output();

    Ok(())
}

#[cfg(all(unix, not(target_os = "macos")))]
fn linux_mime_types() -> String {
    // Map each supported extension to a standard MIME type.
    // Custom/non-standard types use the x- prefix.
    let mime_map: &[(&str, &str)] = &[
        ("avif", "image/avif"),
        ("bmp", "image/bmp"),
        ("dds", "image/x-dds"),
        ("exr", "image/x-exr"),
        ("ff", "image/x-farbfeld"),
        ("farbfeld", "image/x-farbfeld"),
        ("gif", "image/gif"),
        ("hdr", "image/vnd.radiance"),
        ("heic", "image/heic"),
        ("heif", "image/heif"),
        ("ico", "image/vnd.microsoft.icon"),
        ("jpg", "image/jpeg"),
        ("jpeg", "image/jpeg"),
        ("pbm", "image/x-portable-bitmap"),
        ("pam", "image/x-portable-anymap"),
        ("ppm", "image/x-portable-pixmap"),
        ("pgm", "image/x-portable-graymap"),
        ("png", "image/png"),
        ("qoi", "image/x-qoi"),
        ("svg", "image/svg+xml"),
        ("tga", "image/x-tga"),
        ("tif", "image/tiff"),
        ("tiff", "image/tiff"),
        ("webp", "image/webp"),
    ];

    mime_map
        .iter()
        .map(|(_, mime)| *mime)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join(";")
}

#[cfg(all(unix, not(target_os = "macos")))]
fn install_linux_icon(icons_dir: &Path) -> anyhow::Result<()> {
    // Try the sibling resource file first (for installed or dev-environment use).
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    if let Some(base) = &exe_dir {
        let candidate = base.join("icon.png");
        if candidate.exists() {
            std::fs::copy(&candidate, icons_dir.join("lumia.png"))?;
            return Ok(());
        }
        // Also try resources/ relative to the exe (source-tree layout)
        let candidate = base.join("resources/icon.png");
        if candidate.exists() {
            std::fs::copy(&candidate, icons_dir.join("lumia.png"))?;
            return Ok(());
        }
    }

    // Fallback: generate a simple 1-pixel PNG placeholder.
    // The user can replace it later with a proper icon.
    let placeholder: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    std::fs::write(icons_dir.join("lumia.png"), placeholder)?;
    Ok(())
}
