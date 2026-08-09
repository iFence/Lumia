use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use lumia_plugin_api::PluginManifest;
use lumia_plugin_host::{validate_decode_preview_manifest, validate_ui_manifest};
use sha2::{Digest, Sha256};

use crate::plugin_package::{verify_ed25519_signature, OFFICIAL_PLUGIN_PUBLIC_KEY};

const PHOTOSHOP_MANIFEST: &str =
    include_str!("../../../plugins/lumia-plugin-photoshop/lumia.plugin.json");
const JPEG_XL_MANIFEST: &str =
    include_str!("../../../plugins/lumia-plugin-jpeg-xl/lumia.plugin.json");
const JPEG2000_MANIFEST: &str =
    include_str!("../../../plugins/lumia-plugin-jpeg2000/lumia.plugin.json");

/// Decoder plugins shipped inside the Lumia install directory whose manifests
/// are compiled into the binary (`include_str!` above). They have no
/// `lumia.plugin.sig` on disk, so they are trusted because their manifest is
/// part of the signed application itself. Every other plugin must carry a
/// valid Ed25519 signature.
const BUNDLED_PLUGIN_IDS: &[&str] = &["lumia.photoshop", "lumia.jpeg-xl", "lumia.jpeg2000"];

#[derive(Debug, Clone)]
pub(crate) struct InstalledPlugin {
    pub(crate) manifest: PluginManifest,
    pub(crate) root: PathBuf,
}

impl InstalledPlugin {
    pub(crate) fn entry_path(&self) -> PathBuf {
        resolved_entry_path(&self.root, &self.manifest.entry)
    }

    pub(crate) fn asset_path(&self, asset_id: &str) -> Option<PathBuf> {
        self.manifest
            .assets
            .iter()
            .find(|asset| asset.id == asset_id)
            .map(|asset| self.root.join(&asset.path))
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct PluginRegistry {
    plugins: Vec<InstalledPlugin>,
}

impl PluginRegistry {
    pub(crate) fn discover() -> Self {
        let mut roots = plugin_roots();
        if let Ok(development_root) = std::env::var("LUMIA_PLUGIN_DEV_DIR") {
            roots.push(PathBuf::from(development_root));
        }

        let mut plugins = Vec::new();
        for root in roots {
            discover_root(&root, &mut plugins);
        }
        plugins.sort_by(|left, right| left.manifest.id.cmp(&right.manifest.id));
        plugins.dedup_by(|left, right| left.manifest.id == right.manifest.id);
        Self { plugins }
    }

    pub(crate) fn all(&self) -> impl Iterator<Item = &InstalledPlugin> {
        self.plugins.iter()
    }

    pub(crate) fn ui_plugins(&self) -> impl Iterator<Item = &InstalledPlugin> {
        self.plugins
            .iter()
            .filter(|plugin| !plugin.manifest.contributions.commands.is_empty())
    }

    pub(crate) fn decoder_for_extension(&self, extension: &str) -> Option<&InstalledPlugin> {
        self.plugins.iter().find(|plugin| {
            plugin
                .manifest
                .capabilities
                .contains(&lumia_plugin_api::PluginCapability::Probe)
                && plugin
                    .manifest
                    .capabilities
                    .contains(&lumia_plugin_api::PluginCapability::DecodePreview)
                && plugin
                    .manifest
                    .supported_extensions
                    .iter()
                    .any(|candidate| {
                        candidate.eq_ignore_ascii_case(extension.trim_start_matches('.'))
                    })
        })
    }

    pub(crate) fn get(&self, id: &str) -> Option<&InstalledPlugin> {
        self.plugins.iter().find(|plugin| plugin.manifest.id == id)
    }

    pub(crate) fn remove(&mut self, id: &str) {
        self.plugins.retain(|plugin| plugin.manifest.id != id);
    }
}

pub(crate) fn photoshop_manifest() -> Result<PluginManifest> {
    let mut manifest: PluginManifest =
        serde_json::from_str(PHOTOSHOP_MANIFEST).context("parse bundled Photoshop manifest")?;
    validate_decode_preview_manifest(&manifest)
        .context("validate bundled Photoshop plugin capabilities")?;
    let current_exe = std::env::current_exe().context("locate Lumia executable")?;
    manifest.entry = resolve_entry(&current_exe);
    Ok(manifest)
}

pub(crate) fn jpeg_xl_manifest() -> Result<PluginManifest> {
    let mut manifest: PluginManifest =
        serde_json::from_str(JPEG_XL_MANIFEST).context("parse bundled JPEG XL manifest")?;
    validate_decode_preview_manifest(&manifest)
        .context("validate bundled JPEG XL plugin capabilities")?;
    let current_exe = std::env::current_exe().context("locate Lumia executable")?;
    manifest.entry = resolve_named_entry(&current_exe, "lumia-plugin-jpeg-xl");
    Ok(manifest)
}

pub(crate) fn jpeg2000_manifest() -> Result<PluginManifest> {
    let mut manifest: PluginManifest =
        serde_json::from_str(JPEG2000_MANIFEST).context("parse bundled JPEG 2000 manifest")?;
    validate_decode_preview_manifest(&manifest)
        .context("validate bundled JPEG 2000 plugin capabilities")?;
    let current_exe = std::env::current_exe().context("locate Lumia executable")?;
    manifest.entry = resolve_named_entry(&current_exe, "lumia-plugin-jpeg2000");
    Ok(manifest)
}

fn resolve_entry(current_exe: &Path) -> PathBuf {
    entry_candidates(current_exe)
        .into_iter()
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| entry_candidates(current_exe).remove(0))
}

fn resolve_named_entry(current_exe: &Path, executable: &str) -> PathBuf {
    let directory = current_exe.parent().unwrap_or_else(|| Path::new("."));
    let executable = if cfg!(windows) {
        format!("{executable}.exe")
    } else {
        executable.to_string()
    };
    let candidates = [
        directory.join(&executable),
        directory.join("plugins").join(&executable),
        directory
            .join("plugins")
            .join(executable.trim_end_matches(".exe"))
            .join(&executable),
    ];
    candidates
        .iter()
        .find(|candidate| candidate.is_file())
        .cloned()
        .unwrap_or_else(|| candidates[0].clone())
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

fn plugin_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(directory) = current_exe.parent() {
            roots.push(directory.join("plugins"));
        }
    }
    if let Some(user_root) = user_plugin_root() {
        roots.push(user_root);
    }
    roots
}

pub(crate) fn user_plugin_root() -> Option<PathBuf> {
    let application_dir = if cfg!(target_os = "linux") {
        "lumia"
    } else {
        "Lumia"
    };
    dirs::data_dir().map(|data_dir| data_dir.join(application_dir).join("plugins"))
}

fn discover_root(root: &Path, plugins: &mut Vec<InstalledPlugin>) {
    if root.join("lumia.plugin.json").is_file() {
        if let Ok(plugin) = load_official_ui_plugin(root) {
            plugins.push(plugin);
        }
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            if let Ok(plugin) = load_official_ui_plugin(&entry.path()) {
                plugins.push(plugin);
            }
        }
    }
}
fn validate_official_plugin_manifest(manifest: &PluginManifest) -> Result<()> {
    let decoder = manifest
        .capabilities
        .contains(&lumia_plugin_api::PluginCapability::Probe)
        && manifest
            .capabilities
            .contains(&lumia_plugin_api::PluginCapability::DecodePreview);
    let ui = manifest
        .capabilities
        .contains(&lumia_plugin_api::PluginCapability::UiContributions)
        || !manifest.contributions.commands.is_empty()
        || !manifest.contributions.viewer_context_menu.is_empty()
        || !manifest.contributions.right_panels.is_empty()
        || !manifest.contributions.canvas_tools.is_empty();

    if !decoder && !ui {
        anyhow::bail!("plugin declares neither decoder nor UI capabilities");
    }
    if decoder {
        validate_decode_preview_manifest(manifest).context("validate decoder capabilities")?;
        if manifest.supported_extensions.is_empty() {
            anyhow::bail!("decoder plugin declares no supported extensions");
        }
    }
    if ui {
        validate_ui_manifest(manifest).context("validate UI contributions")?;
    }
    Ok(())
}

pub(crate) fn load_official_ui_plugin(root: &Path) -> Result<InstalledPlugin> {
    let manifest_path = root.join("lumia.plugin.json");
    let manifest_bytes =
        fs::read(&manifest_path).with_context(|| format!("read {}", manifest_path.display()))?;
    let manifest: PluginManifest =
        serde_json::from_slice(&manifest_bytes).context("parse plugin manifest")?;
    match verify_manifest_signature(root, &manifest_bytes) {
        Ok(()) => {}
        Err(signature_error) if !is_trusted_bundled(root, &manifest) => {
            return Err(signature_error.context(format!(
                "plugin {} is not signed by the official Lumia key",
                manifest.id
            )));
        }
        Err(_) => {}
    }
    validate_official_plugin_manifest(&manifest)?;
    let entry = resolved_entry_path(root, &manifest.entry);
    if !entry.is_file() {
        anyhow::bail!("plugin entry is missing: {}", entry.display());
    }
    let canonical_root = root
        .canonicalize()
        .context("canonicalize plugin directory")?;
    let canonical_entry = entry.canonicalize().context("canonicalize plugin entry")?;
    if !canonical_entry.starts_with(&canonical_root) {
        anyhow::bail!("plugin entry escapes its package");
    }
    verify_assets(root, &manifest)?;
    Ok(InstalledPlugin {
        manifest,
        root: root.to_path_buf(),
    })
}

/// `true` for the decoder plugins shipped inside the Lumia install directory
/// (`<exe>/plugins/<name>/`) whose manifests are compiled into the binary via
/// `include_str!`. Those carry no `lumia.plugin.sig` on disk, so their trust
/// anchor is the application binary itself. The location guard restricts the
/// bypass to the bundled install location: a directory a user could write to
/// (for example a package installed under the per-user plugins root) can never
/// skip signature verification, even if it claims a bundled plugin id.
fn is_trusted_bundled(root: &Path, manifest: &PluginManifest) -> bool {
    if !BUNDLED_PLUGIN_IDS.contains(&manifest.id.as_str()) {
        return false;
    }
    let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|current| current.parent().map(Path::to_path_buf))
    else {
        return false;
    };
    root.starts_with(&exe_dir.join("plugins"))
}

fn verify_manifest_signature(root: &Path, manifest_bytes: &[u8]) -> Result<()> {
    let signature_path = root.join("lumia.plugin.sig");
    let signature_text = fs::read_to_string(&signature_path)
        .with_context(|| format!("read {}", signature_path.display()))?;
    verify_ed25519_signature(
        &OFFICIAL_PLUGIN_PUBLIC_KEY,
        manifest_bytes,
        signature_text.trim(),
    )
    .map_err(anyhow::Error::from)
}

fn verify_assets(root: &Path, manifest: &PluginManifest) -> Result<()> {
    let canonical_root = root
        .canonicalize()
        .context("canonicalize plugin directory")?;
    for asset in &manifest.assets {
        let path = root.join(&asset.path);
        let canonical_path = path
            .canonicalize()
            .with_context(|| format!("locate plugin asset {}", asset.id))?;
        if !canonical_path.starts_with(&canonical_root) {
            anyhow::bail!("plugin asset {} escapes its package", asset.id);
        }
        let bytes = fs::read(&canonical_path)?;
        let digest = hex::encode(Sha256::digest(bytes));
        if digest != asset.sha256 {
            anyhow::bail!("plugin asset {} failed integrity validation", asset.id);
        }
    }
    Ok(())
}

fn resolved_entry_path(root: &Path, entry: &Path) -> PathBuf {
    let entry = root.join(entry);
    if cfg!(windows) && entry.extension().is_none() {
        entry.with_extension("exe")
    } else {
        entry
    }
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
    fn jpeg_xl_manifest_declares_safe_preview_contract() {
        let manifest: PluginManifest = serde_json::from_str(JPEG_XL_MANIFEST).unwrap();
        assert_eq!(manifest.id, "lumia.jpeg-xl");
        validate_decode_preview_manifest(&manifest).unwrap();
        assert_eq!(manifest.supported_inputs, ["image/jxl"]);
        assert_eq!(manifest.supported_outputs, ["image/png"]);
    }

    #[test]
    fn jpeg2000_manifest_declares_safe_preview_contract() {
        let manifest: PluginManifest = serde_json::from_str(JPEG2000_MANIFEST).unwrap();
        assert_eq!(manifest.id, "lumia.jpeg2000");
        validate_decode_preview_manifest(&manifest).unwrap();
        assert_eq!(manifest.supported_extensions, ["jp2", "j2k", "j2c", "jpc"]);
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

    #[test]
    fn annotation_package_signature_and_assets_are_valid() {
        let root =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins/lumia-plugin-annotation");
        let manifest_bytes = fs::read(root.join("lumia.plugin.json")).unwrap();
        verify_manifest_signature(&root, &manifest_bytes).unwrap();
        let manifest: PluginManifest = serde_json::from_slice(&manifest_bytes).unwrap();
        validate_ui_manifest(&manifest).unwrap();
        verify_assets(&root, &manifest).unwrap();
    }

    #[test]
    fn entry_resolution_adds_windows_suffix_only_when_needed() {
        let path = resolved_entry_path(Path::new("plugins/annotation"), Path::new("plugin"));
        if cfg!(windows) {
            assert_eq!(
                path.extension().and_then(|value| value.to_str()),
                Some("exe")
            );
        } else {
            assert!(path.extension().is_none());
        }
    }

    #[test]
    fn unsigned_third_party_plugin_is_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let manifest = serde_json::to_vec(&unsigned_manifest("com.example.foo")).unwrap();
        fs::write(directory.path().join("lumia.plugin.json"), manifest).unwrap();
        assert!(
            load_official_ui_plugin(directory.path()).is_err(),
            "unsigned third-party plugin must be rejected"
        );
    }

    #[test]
    fn bundled_decoder_id_outside_install_dir_is_rejected_without_signature() {
        let directory = tempfile::tempdir().unwrap();
        let manifest = serde_json::to_vec(&unsigned_manifest("lumia.photoshop")).unwrap();
        fs::write(directory.path().join("lumia.plugin.json"), manifest).unwrap();
        assert!(
            load_official_ui_plugin(directory.path()).is_err(),
            "bundled id outside the bundled install directory must not bypass signature verification"
        );
    }

    fn unsigned_manifest(id: &str) -> PluginManifest {
        PluginManifest {
            id: id.into(),
            name: id.into(),
            version: "0.1.0".into(),
            entry: PathBuf::from("plugin"),
            capabilities: vec![
                lumia_plugin_api::PluginCapability::Probe,
                lumia_plugin_api::PluginCapability::DecodePreview,
            ],
            permissions: vec![
                lumia_plugin_api::PluginPermission::ReadInputPath,
                lumia_plugin_api::PluginPermission::WriteTemporaryOutput,
            ],
            supported_inputs: vec!["image/x-test".into()],
            supported_extensions: vec!["tst".into()],
            supported_outputs: vec!["image/png".into()],
            contributions: Default::default(),
            assets: Vec::new(),
        }
    }
}
