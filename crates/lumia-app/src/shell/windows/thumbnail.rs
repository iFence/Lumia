//! Registration of Lumia's SVG thumbnail provider with Windows Explorer.
//!
//! Explorer renders thumbnails for an extension through an `IThumbnailProvider`
//! COM server registered under `HKCU\Software\Classes\.ext\shellex\ThumbnailHandler`.
//! Lumia ships `lumia_svg_thumbnail.dll` next to the main executable and points
//! the shell at it here. Everything lives under HKCU, so no administrator
//! rights are required.

use std::io::ErrorKind;
use std::path::Path;

use anyhow::{bail, Context as _};
use winreg::enums::*;
use winreg::RegKey;

use super::{normalize_windows_path, notify_associations_changed};

/// COM class id of `lumia_svg_thumbnail.dll`, as a registry string. Must match
/// `CLSID_SVG_THUMBNAIL` in `crates/lumia-svg-thumbnail/src/com.rs`.
const THUMBNAIL_CLSID: &str = "{0F6F22C8-3077-4B32-A61C-7738E61F242B}";
/// Name of the thumbnail-provider DLL, installed next to `lumia-app.exe`.
const THUMBNAIL_DLL: &str = "lumia_svg_thumbnail.dll";
/// Extensions handed to the provider. `.svgz` is a gzip-compressed SVG, which
/// the provider's parser accepts directly.
const THUMBNAIL_EXTENSIONS: [&str; 2] = ["svg", "svgz"];

/// Point Explorer's SVG thumbnail handler at Lumia's provider DLL.
pub(super) fn register_thumbnail_handler(exe_dir: &Path) -> anyhow::Result<()> {
    let dll_path = exe_dir.join(THUMBNAIL_DLL);
    if !dll_path.is_file() {
        bail!(
            "thumbnail provider DLL not found at {}; keep {} next to lumia-app.exe",
            dll_path.display(),
            THUMBNAIL_DLL
        );
    }

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    let clsid_key = format!(r"Software\Classes\CLSID\{THUMBNAIL_CLSID}\InprocServer32");
    let (key, _) = hkcu
        .create_subkey(&clsid_key)
        .with_context(|| format!("create registry key {clsid_key}"))?;
    key.set_value("", &dll_path.to_string_lossy().to_string())
        .with_context(|| format!("set {clsid_key}\\ (default)"))?;
    key.set_value("ThreadingModel", &"Apartment".to_string())
        .with_context(|| format!("set {clsid_key}\\ThreadingModel"))?;

    for extension in THUMBNAIL_EXTENSIONS {
        let handler = format!(r"Software\Classes\.{extension}\shellex\ThumbnailHandler");
        let (key, _) = hkcu
            .create_subkey(&handler)
            .with_context(|| format!("create registry key {handler}"))?;
        key.set_value("", &THUMBNAIL_CLSID.to_string())
            .with_context(|| format!("set {handler}\\ (default)"))?;
    }

    notify_associations_changed();
    Ok(())
}

/// Remove Lumia's SVG thumbnail handler from Explorer.
pub(super) fn unregister_thumbnail_handler() -> anyhow::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    let clsid_tree = format!(r"Software\Classes\CLSID\{THUMBNAIL_CLSID}");
    if let Err(error) = hkcu.delete_subkey_all(&clsid_tree) {
        if error.kind() != ErrorKind::NotFound {
            return Err(error).with_context(|| format!("delete registry key {clsid_tree}"));
        }
    }

    for extension in THUMBNAIL_EXTENSIONS {
        let handler = format!(r"Software\Classes\.{extension}\shellex\ThumbnailHandler");
        if let Err(error) = hkcu.delete_subkey_all(&handler) {
            if error.kind() != ErrorKind::NotFound {
                return Err(error).with_context(|| format!("delete registry key {handler}"));
            }
        }
    }

    notify_associations_changed();
    Ok(())
}

/// Whether Explorer's SVG thumbnail handler is currently pointed at the DLL in
/// `exe_dir`. Path-aware so a portable install that moves directories re-registers
/// itself.
pub(super) fn thumbnail_handler_configured(exe_dir: &Path) -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    let handler = hkcu
        .open_subkey(r"Software\Classes\.svg\shellex\ThumbnailHandler")
        .ok()
        .and_then(|key| key.get_value::<String, _>("").ok())
        .map(|value| normalize_guid(&value));
    if handler.as_deref() != Some(THUMBNAIL_CLSID) {
        return false;
    }

    let expected = normalize_windows_path(&exe_dir.join(THUMBNAIL_DLL).to_string_lossy());
    let inproc = hkcu
        .open_subkey(format!(r"Software\Classes\CLSID\{THUMBNAIL_CLSID}\InprocServer32"))
        .ok()
        .and_then(|key| key.get_value::<String, _>("").ok())
        .map(|value| normalize_windows_path(&value));
    inproc.as_deref() == Some(expected.as_str())
}

fn normalize_guid(value: &str) -> String {
    value.trim_matches(|c| c == '{' || c == '}').to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_comparison_is_brace_and_case_insensitive() {
        assert_eq!(normalize_guid("{0f6f22c8-3077-4B32-a61c-7738e61f242b}"), normalize_guid(THUMBNAIL_CLSID));
        assert_eq!(normalize_guid("0F6F22C8-3077-4B32-A61C-7738E61F242B"), normalize_guid(THUMBNAIL_CLSID));
        assert_ne!(normalize_guid("{11111111-2222-3333-4444-555555555555}"), normalize_guid(THUMBNAIL_CLSID));
    }

    #[test]
    fn registered_extensions_cover_svg_and_svgz() {
        assert_eq!(THUMBNAIL_EXTENSIONS, ["svg", "svgz"]);
    }
}
