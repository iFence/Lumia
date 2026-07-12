use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use lumia_plugin_api::PluginManifest;
use lumia_plugin_host::validate_decode_preview_manifest;

const PHOTOSHOP_MANIFEST: &str =
    include_str!("../../../plugins/lumia-plugin-photoshop/lumia.plugin.json");

pub(crate) fn photoshop_manifest() -> Result<PluginManifest> {
    let mut manifest: PluginManifest =
        serde_json::from_str(PHOTOSHOP_MANIFEST).context("parse bundled Photoshop manifest")?;
    validate_decode_preview_manifest(&manifest)
        .context("validate bundled Photoshop plugin capabilities")?;
    let current_exe = std::env::current_exe().context("locate Lumia executable")?;
    manifest.entry = resolve_entry(&current_exe);
    Ok(manifest)
}

fn resolve_entry(current_exe: &Path) -> PathBuf {
    entry_candidates(current_exe)
        .into_iter()
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| entry_candidates(current_exe).remove(0))
}

fn entry_candidates(current_exe: &Path) -> Vec<PathBuf> {
    let directory = current_exe.parent().unwrap_or_else(|| Path::new("."));
    let executable = if cfg!(windows) {
        "lumia-plugin-photoshop.exe"
    } else {
        "lumia-plugin-photoshop"
    };
    vec![
        directory.join(executable),
        directory.join("plugins").join(executable),
        directory
            .join("plugins")
            .join("lumia-plugin-photoshop")
            .join(executable),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_manifest_declares_safe_preview_contract() {
        let manifest: PluginManifest = serde_json::from_str(PHOTOSHOP_MANIFEST).unwrap();
        assert_eq!(manifest.id, "lumia.photoshop");
        validate_decode_preview_manifest(&manifest).unwrap();
        assert_eq!(manifest.supported_inputs, ["image/vnd.adobe.photoshop"]);
        assert_eq!(manifest.supported_outputs, ["image/png"]);
    }

    #[test]
    fn entry_candidates_prefer_executable_sibling() {
        let executable = Path::new(r"C:\Lumia\lumia-app.exe");
        let candidates = entry_candidates(executable);
        assert_eq!(
            candidates[0].file_stem().and_then(|name| name.to_str()),
            Some("lumia-plugin-photoshop")
        );
    }
}
