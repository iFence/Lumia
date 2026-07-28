use std::fs::File;
use std::io::Read as _;

use lumia_plugin_api::PluginManifest;
use zip::ZipArchive;

use super::{
    io_error, verify_ed25519_signature, PluginPackageError, VerifiedPluginPackage,
    MAX_METADATA_BYTES, OFFICIAL_PLUGIN_PUBLIC_KEY,
};

pub(crate) fn inspect_packaged_plugin_manifest(
    package: &VerifiedPluginPackage,
) -> Result<PluginManifest, PluginPackageError> {
    let file = File::open(&package.archive_path).map_err(io_error)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| PluginPackageError::InvalidArchive(error.to_string()))?;
    let manifest_path = format!("{}/lumia.plugin.json", package.manifest.install_directory);
    let signature_path = format!("{}/lumia.plugin.sig", package.manifest.install_directory);
    let manifest_bytes = read_bounded_entry(&mut archive, &manifest_path)?;
    let signature_bytes = read_bounded_entry(&mut archive, &signature_path)?;
    let signature_text = std::str::from_utf8(&signature_bytes)
        .map_err(|_| PluginPackageError::InvalidSignatureEncoding)?;
    verify_ed25519_signature(&OFFICIAL_PLUGIN_PUBLIC_KEY, &manifest_bytes, signature_text)?;
    let manifest: PluginManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| PluginPackageError::InvalidPluginManifest(error.to_string()))?;
    if manifest.id != package.manifest.plugin_id || manifest.version != package.manifest.version {
        return Err(PluginPackageError::PluginManifestMismatch);
    }
    Ok(manifest)
}

fn read_bounded_entry(
    archive: &mut ZipArchive<File>,
    name: &str,
) -> Result<Vec<u8>, PluginPackageError> {
    let mut entry = archive
        .by_name(name)
        .map_err(|_| PluginPackageError::MissingPayload(name.into()))?;
    if entry.size() > MAX_METADATA_BYTES {
        return Err(PluginPackageError::FileTooLarge(name.into()));
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut bytes).map_err(io_error)?;
    Ok(bytes)
}
