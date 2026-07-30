use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};

use base64::Engine as _;
use ring::signature::{UnparsedPublicKey, ED25519};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use zip::ZipArchive;
mod runtime;
pub(crate) use runtime::inspect_packaged_plugin_manifest;

const PACKAGE_SCHEMA_VERSION: u32 = 1;
const OFFICIAL_PLUGIN_IDS: &[&str] = &["lumia.annotation", "lumia.raw"];
const PACKAGE_MANIFEST_NAME: &str = "lumia.package.json";
const PACKAGE_SIGNATURE_NAME: &str = "lumia.package.sig";
const MAX_COMPRESSED_PACKAGE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_TOTAL_PAYLOAD_BYTES: u64 = 512 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;
const MAX_METADATA_BYTES: u64 = 1024 * 1024;
const MAX_PAYLOAD_FILES: usize = 512;
const MAX_ARCHIVE_ENTRIES: usize = MAX_PAYLOAD_FILES + 64;
const MAX_PATH_BYTES: usize = 240;
const HASH_BUFFER_BYTES: usize = 64 * 1024;

pub(crate) const OFFICIAL_PLUGIN_PUBLIC_KEY: [u8; 32] = [
    0x6b, 0x88, 0xde, 0x1c, 0x86, 0xa7, 0x3a, 0xe6, 0x66, 0xd4, 0xa4, 0x4b, 0x54, 0xe3, 0x04, 0x69,
    0x00, 0xff, 0x24, 0xa0, 0x85, 0xa2, 0x51, 0x5a, 0xda, 0x36, 0xd2, 0xb1, 0x5c, 0xc5, 0x54, 0x17,
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PluginPackageManifest {
    pub(crate) schema_version: u32,
    pub(crate) plugin_id: String,
    pub(crate) version: String,
    pub(crate) plugin_api_version: u32,
    pub(crate) minimum_lumia_version: String,
    pub(crate) target_os: String,
    pub(crate) target_arch: String,
    pub(crate) install_directory: String,
    pub(crate) files: Vec<PluginPackageFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PluginPackageFile {
    pub(crate) path: String,
    pub(crate) size: u64,
    pub(crate) sha256: String,
}

#[derive(Debug, Clone)]
pub(crate) struct VerifiedPluginPackage {
    pub(crate) archive_path: PathBuf,
    pub(crate) manifest: PluginPackageManifest,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub(crate) enum PluginPackageError {
    #[error("unsupported plugin package schema {0}")]
    UnsupportedSchema(u32),
    #[error("plugin {0} is not an allowlisted official plugin")]
    UnofficialPlugin(String),
    #[error("invalid semantic version in {field}: {value}")]
    InvalidVersion { field: &'static str, value: String },
    #[error("package targets {actual}, but this Lumia build targets {expected}")]
    IncompatiblePlatform { expected: String, actual: String },
    #[error("package targets architecture {actual}, but this Lumia build targets {expected}")]
    IncompatibleArchitecture { expected: String, actual: String },
    #[error("package requires plugin API {actual}, but Lumia supports {expected}")]
    IncompatiblePluginApi { expected: u32, actual: u32 },
    #[error("package requires Lumia {minimum} or newer, but this build is {current}")]
    IncompatibleLumiaVersion { minimum: String, current: String },
    #[error("cannot downgrade plugin from {installed} to {package}")]
    DowngradeBlocked { installed: String, package: String },
    #[error("unsafe plugin installation directory {0:?}")]
    UnsafeInstallDirectory(String),
    #[error("plugin package is too large")]
    PackageTooLarge,
    #[error("plugin package contains too many entries")]
    TooManyEntries,
    #[error("missing package metadata file {0}")]
    MissingMetadata(&'static str),
    #[error("package metadata file {0} is too large")]
    MetadataTooLarge(&'static str),
    #[error("invalid package manifest: {0}")]
    InvalidManifest(String),
    #[error("invalid packaged plugin manifest: {0}")]
    InvalidPluginManifest(String),
    #[error("packaged plugin manifest does not match the package metadata")]
    PluginManifestMismatch,
    #[error("invalid package signature encoding")]
    InvalidSignatureEncoding,
    #[error("plugin package signature is invalid")]
    InvalidSignature,
    #[error("unsafe package path {0:?}")]
    UnsafePath(String),
    #[error("duplicate or case-colliding package path {0:?}")]
    DuplicatePath(String),
    #[error("encrypted package entry {0:?} is not supported")]
    EncryptedEntry(String),
    #[error("link or unsupported package entry {0:?} is not allowed")]
    UnsupportedEntryType(String),
    #[error("plugin package declares too many payload files")]
    TooManyPayloadFiles,
    #[error("payload file {0:?} exceeds its size limit")]
    FileTooLarge(String),
    #[error("plugin package payload exceeds its total size limit")]
    PayloadTooLarge,
    #[error("invalid SHA-256 for payload file {0:?}")]
    InvalidDigest(String),
    #[error("missing declared payload file {0:?}")]
    MissingPayload(String),
    #[error("undeclared payload file {0:?}")]
    UnexpectedPayload(String),
    #[error("payload size mismatch for {0:?}")]
    SizeMismatch(String),
    #[error("payload hash mismatch for {0:?}")]
    HashMismatch(String),
    #[error("invalid ZIP archive: {0}")]
    InvalidArchive(String),
    #[error("failed to read plugin package: {0}")]
    Io(String),
}

pub(crate) fn inspect_official_package(
    archive_path: &Path,
    installed_version: Option<&str>,
) -> Result<VerifiedPluginPackage, PluginPackageError> {
    inspect_package_with_key(archive_path, installed_version, &OFFICIAL_PLUGIN_PUBLIC_KEY)
}
pub(crate) fn verify_official_package_file(
    archive_path: &Path,
) -> Result<VerifiedPluginPackage, PluginPackageError> {
    let package = inspect_official_package(archive_path, None)?;
    inspect_packaged_plugin_manifest(&package)?;
    Ok(package)
}

fn inspect_package_with_key(
    archive_path: &Path,
    installed_version: Option<&str>,
    public_key: &[u8],
) -> Result<VerifiedPluginPackage, PluginPackageError> {
    let metadata = fs::metadata(archive_path).map_err(io_error)?;
    if metadata.len() > MAX_COMPRESSED_PACKAGE_BYTES {
        return Err(PluginPackageError::PackageTooLarge);
    }
    let file = File::open(archive_path).map_err(io_error)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| PluginPackageError::InvalidArchive(error.to_string()))?;
    validate_archive_entries(&mut archive)?;

    let manifest_bytes = read_metadata(&mut archive, PACKAGE_MANIFEST_NAME)?;
    let signature_bytes = read_metadata(&mut archive, PACKAGE_SIGNATURE_NAME)?;
    let signature_text = std::str::from_utf8(&signature_bytes)
        .map_err(|_| PluginPackageError::InvalidSignatureEncoding)?;
    verify_ed25519_signature(public_key, &manifest_bytes, signature_text)?;

    let manifest: PluginPackageManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| PluginPackageError::InvalidManifest(error.to_string()))?;
    validate_compatibility(&manifest, installed_version)?;
    let declarations = validate_declarations(&manifest)?;
    verify_payload(&mut archive, &manifest, &declarations)?;

    Ok(VerifiedPluginPackage {
        archive_path: archive_path.to_path_buf(),
        manifest,
    })
}

pub(crate) fn verify_ed25519_signature(
    public_key: &[u8],
    message: &[u8],
    signature_text: &str,
) -> Result<(), PluginPackageError> {
    let signature = base64::engine::general_purpose::STANDARD
        .decode(signature_text.trim())
        .map_err(|_| PluginPackageError::InvalidSignatureEncoding)?;
    UnparsedPublicKey::new(&ED25519, public_key)
        .verify(message, &signature)
        .map_err(|_| PluginPackageError::InvalidSignature)
}

pub(crate) fn validate_compatibility(
    manifest: &PluginPackageManifest,
    installed_version: Option<&str>,
) -> Result<(), PluginPackageError> {
    if manifest.schema_version != PACKAGE_SCHEMA_VERSION {
        return Err(PluginPackageError::UnsupportedSchema(
            manifest.schema_version,
        ));
    }
    if !is_official_plugin_id(&manifest.plugin_id) {
        return Err(PluginPackageError::UnofficialPlugin(
            manifest.plugin_id.clone(),
        ));
    }
    validate_install_directory(&manifest.install_directory)?;

    let package_version = parse_version("version", &manifest.version)?;
    let minimum_lumia = parse_version("minimum_lumia_version", &manifest.minimum_lumia_version)?;
    let current_lumia = parse_version("Lumia version", env!("CARGO_PKG_VERSION"))?;

    if manifest.target_os != current_target_os() {
        return Err(PluginPackageError::IncompatiblePlatform {
            expected: current_target_os().into(),
            actual: manifest.target_os.clone(),
        });
    }
    if manifest.target_arch != current_target_arch() {
        return Err(PluginPackageError::IncompatibleArchitecture {
            expected: current_target_arch().into(),
            actual: manifest.target_arch.clone(),
        });
    }
    if manifest.plugin_api_version != lumia_plugin_api::PROTOCOL_VERSION {
        return Err(PluginPackageError::IncompatiblePluginApi {
            expected: lumia_plugin_api::PROTOCOL_VERSION,
            actual: manifest.plugin_api_version,
        });
    }
    if minimum_lumia > current_lumia {
        return Err(PluginPackageError::IncompatibleLumiaVersion {
            minimum: manifest.minimum_lumia_version.clone(),
            current: env!("CARGO_PKG_VERSION").into(),
        });
    }
    if let Some(installed) = installed_version {
        let installed_version = parse_version("installed version", installed)?;
        if package_version < installed_version {
            return Err(PluginPackageError::DowngradeBlocked {
                installed: installed.into(),
                package: manifest.version.clone(),
            });
        }
    }
    Ok(())
}

pub(crate) fn is_official_plugin_id(id: &str) -> bool {
    OFFICIAL_PLUGIN_IDS.contains(&id)
}

pub(crate) const fn current_target_os() -> &'static str {
    std::env::consts::OS
}

pub(crate) const fn current_target_arch() -> &'static str {
    std::env::consts::ARCH
}

fn validate_archive_entries(archive: &mut ZipArchive<File>) -> Result<(), PluginPackageError> {
    if archive.len() > MAX_ARCHIVE_ENTRIES {
        return Err(PluginPackageError::TooManyEntries);
    }
    let mut paths = HashSet::new();
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|error| PluginPackageError::InvalidArchive(error.to_string()))?;
        let name = entry.name().to_string();
        validate_archive_path(&name, entry.is_dir())?;
        let comparison = path_comparison_key(&name);
        if !paths.insert(comparison) {
            return Err(PluginPackageError::DuplicatePath(name));
        }
        if entry.encrypted() {
            return Err(PluginPackageError::EncryptedEntry(name));
        }
        if entry.is_symlink() || (!entry.is_dir() && !entry.is_file()) {
            return Err(PluginPackageError::UnsupportedEntryType(name));
        }
    }
    Ok(())
}

fn read_metadata(
    archive: &mut ZipArchive<File>,
    name: &'static str,
) -> Result<Vec<u8>, PluginPackageError> {
    let mut entry = archive
        .by_name(name)
        .map_err(|_| PluginPackageError::MissingMetadata(name))?;
    if entry.size() > MAX_METADATA_BYTES {
        return Err(PluginPackageError::MetadataTooLarge(name));
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut bytes).map_err(io_error)?;
    Ok(bytes)
}

fn validate_declarations(
    manifest: &PluginPackageManifest,
) -> Result<HashMap<String, &PluginPackageFile>, PluginPackageError> {
    if manifest.files.len() > MAX_PAYLOAD_FILES {
        return Err(PluginPackageError::TooManyPayloadFiles);
    }
    let mut files = HashMap::new();
    let mut comparison_paths = HashSet::new();
    let mut total = 0_u64;
    for file in &manifest.files {
        validate_archive_path(&file.path, false)?;
        let relative = Path::new(&file.path)
            .strip_prefix(&manifest.install_directory)
            .map_err(|_| PluginPackageError::UnsafePath(file.path.clone()))?;
        if relative.as_os_str().is_empty() {
            return Err(PluginPackageError::UnsafePath(file.path.clone()));
        }
        if file.size > MAX_FILE_BYTES {
            return Err(PluginPackageError::FileTooLarge(file.path.clone()));
        }
        total = total
            .checked_add(file.size)
            .ok_or(PluginPackageError::PayloadTooLarge)?;
        if total > MAX_TOTAL_PAYLOAD_BYTES {
            return Err(PluginPackageError::PayloadTooLarge);
        }
        if file.sha256.len() != 64
            || !file
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err(PluginPackageError::InvalidDigest(file.path.clone()));
        }
        if !comparison_paths.insert(path_comparison_key(&file.path)) {
            return Err(PluginPackageError::DuplicatePath(file.path.clone()));
        }
        files.insert(file.path.clone(), file);
    }
    Ok(files)
}

fn verify_payload(
    archive: &mut ZipArchive<File>,
    manifest: &PluginPackageManifest,
    declarations: &HashMap<String, &PluginPackageFile>,
) -> Result<(), PluginPackageError> {
    let mut found = HashSet::new();
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| PluginPackageError::InvalidArchive(error.to_string()))?;
        let name = entry.name().to_string();
        if matches!(
            name.as_str(),
            PACKAGE_MANIFEST_NAME | PACKAGE_SIGNATURE_NAME
        ) || entry.is_dir()
        {
            continue;
        }
        if !Path::new(&name).starts_with(&manifest.install_directory) {
            return Err(PluginPackageError::UnexpectedPayload(name));
        }
        let Some(declared) = declarations.get(&name) else {
            return Err(PluginPackageError::UnexpectedPayload(name));
        };
        if entry.size() != declared.size {
            return Err(PluginPackageError::SizeMismatch(name));
        }
        let digest = hash_reader(&mut entry, declared.size, &name)?;
        if digest != declared.sha256 {
            return Err(PluginPackageError::HashMismatch(name));
        }
        found.insert(name);
    }
    for path in declarations.keys() {
        if !found.contains(path) {
            return Err(PluginPackageError::MissingPayload(path.clone()));
        }
    }
    Ok(())
}

fn hash_reader(
    reader: &mut impl Read,
    expected_size: u64,
    path: &str,
) -> Result<String, PluginPackageError> {
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; HASH_BUFFER_BYTES];
    let mut total = 0_u64;
    loop {
        let read = reader.read(&mut buffer).map_err(io_error)?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if total > expected_size {
            return Err(PluginPackageError::SizeMismatch(path.into()));
        }
        hasher.update(&buffer[..read]);
    }
    if total != expected_size {
        return Err(PluginPackageError::SizeMismatch(path.into()));
    }
    Ok(hex::encode(hasher.finalize()))
}

fn validate_archive_path(value: &str, directory: bool) -> Result<(), PluginPackageError> {
    if value.is_empty()
        || value.len() > MAX_PATH_BYTES
        || value.contains(['\\', '\0', ':'])
        || value.starts_with('/')
    {
        return Err(PluginPackageError::UnsafePath(value.into()));
    }
    let normalized = if directory {
        value.trim_end_matches('/')
    } else {
        value
    };
    if normalized.is_empty() || (!directory && value.ends_with('/')) {
        return Err(PluginPackageError::UnsafePath(value.into()));
    }
    let path = Path::new(normalized);
    if path.is_absolute() {
        return Err(PluginPackageError::UnsafePath(value.into()));
    }
    for component in path.components() {
        let Component::Normal(component) = component else {
            return Err(PluginPackageError::UnsafePath(value.into()));
        };
        let Some(component) = component.to_str() else {
            return Err(PluginPackageError::UnsafePath(value.into()));
        };
        if component.is_empty()
            || component.ends_with(['.', ' '])
            || is_windows_reserved_name(component)
        {
            return Err(PluginPackageError::UnsafePath(value.into()));
        }
    }
    Ok(())
}

fn is_windows_reserved_name(component: &str) -> bool {
    let stem = component
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|number| {
                matches!(number, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
            })
}

fn path_comparison_key(value: &str) -> String {
    value.trim_end_matches('/').to_lowercase()
}

fn parse_version(field: &'static str, value: &str) -> Result<Version, PluginPackageError> {
    Version::parse(value).map_err(|_| PluginPackageError::InvalidVersion {
        field,
        value: value.into(),
    })
}

fn validate_install_directory(value: &str) -> Result<(), PluginPackageError> {
    validate_archive_path(value, false)
        .map_err(|_| PluginPackageError::UnsafeInstallDirectory(value.into()))?;
    let mut components = Path::new(value).components();
    if matches!(components.next(), Some(Component::Normal(_))) && components.next().is_none() {
        Ok(())
    } else {
        Err(PluginPackageError::UnsafeInstallDirectory(value.into()))
    }
}

fn io_error(error: io::Error) -> PluginPackageError {
    PluginPackageError::Io(error.to_string())
}

#[cfg(test)]
mod tests;
