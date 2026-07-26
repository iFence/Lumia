use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use base64::Engine as _;
use lumia_plugin_api::PluginManifest;
use lumia_plugin_host::{validate_decode_preview_manifest, validate_ui_manifest};
use ring::signature::{UnparsedPublicKey, ED25519};
use sha2::{Digest, Sha256};

const PHOTOSHOP_MANIFEST: &str =
    include_str!("../../../plugins/lumia-plugin-photoshop/lumia.plugin.json");
const OFFICIAL_PLUGIN_PUBLIC_KEY: [u8; 32] = [
    0x09, 0x82, 0x0c, 0xc2, 0x24, 0x31, 0x21, 0xfe, 0x1d, 0x00, 0x51, 0x4f, 0xa4, 0xdf, 0xfb, 0xd5,
    0x1d, 0x21, 0xcc, 0x75, 0x8a, 0x51, 0x86, 0x66, 0x4c, 0x24, 0xba, 0xb4, 0x8e, 0x55, 0x06, 0x2f,
];
const OFFICIAL_UI_PLUGIN_IDS: &[&str] = &["lumia.annotation"];

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

    pub(crate) fn ui_plugins(&self) -> impl Iterator<Item = &InstalledPlugin> {
        self.plugins
            .iter()
            .filter(|plugin| !plugin.manifest.contributions.commands.is_empty())
    }

    pub(crate) fn get(&self, id: &str) -> Option<&InstalledPlugin> {
        self.plugins.iter().find(|plugin| plugin.manifest.id == id)
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

fn plugin_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(directory) = current_exe.parent() {
            roots.push(directory.join("plugins"));
        }
    }
    if let Some(data_dir) = dirs::data_dir() {
        let application_dir = if cfg!(target_os = "linux") {
            "lumia"
        } else {
            "Lumia"
        };
        roots.push(data_dir.join(application_dir).join("plugins"));
    }
    roots
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

fn load_official_ui_plugin(root: &Path) -> Result<InstalledPlugin> {
    let manifest_path = root.join("lumia.plugin.json");
    let manifest_bytes =
        fs::read(&manifest_path).with_context(|| format!("read {}", manifest_path.display()))?;
    verify_manifest_signature(root, &manifest_bytes)?;
    let manifest: PluginManifest =
        serde_json::from_slice(&manifest_bytes).context("parse plugin manifest")?;
    if !OFFICIAL_UI_PLUGIN_IDS.contains(&manifest.id.as_str()) {
        anyhow::bail!("plugin {} is not in the official allowlist", manifest.id);
    }
    validate_ui_manifest(&manifest).context("validate UI contributions")?;
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

fn verify_manifest_signature(root: &Path, manifest_bytes: &[u8]) -> Result<()> {
    let signature_path = root.join("lumia.plugin.sig");
    let signature_text = fs::read_to_string(&signature_path)
        .with_context(|| format!("read {}", signature_path.display()))?;
    let signature = base64::engine::general_purpose::STANDARD
        .decode(signature_text.trim())
        .context("decode plugin signature")?;
    UnparsedPublicKey::new(&ED25519, OFFICIAL_PLUGIN_PUBLIC_KEY)
        .verify(manifest_bytes, &signature)
        .map_err(|_| anyhow::anyhow!("plugin manifest signature is invalid"))
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
}
