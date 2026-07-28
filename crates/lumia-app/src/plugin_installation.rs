use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use lumia_plugin_api::PluginManifest;
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;
use zip::ZipArchive;

use crate::plugin_catalog::load_official_ui_plugin;
use crate::plugin_package::{validate_compatibility, PluginPackageError, VerifiedPluginPackage};

const STAGING_DIRECTORY: &str = ".staging";
const BACKUP_DIRECTORY: &str = ".backup";
const REMOVING_DIRECTORY: &str = ".removing";
const COPY_BUFFER_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InstallOutcome {
    pub(crate) plugin_id: String,
    pub(crate) previous_version: Option<String>,
    pub(crate) installed_version: String,
    pub(crate) restart_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UninstallOutcome {
    pub(crate) install_directory: String,
    pub(crate) restart_required: bool,
}

#[derive(Debug, Error)]
pub(crate) enum PluginInstallationError {
    #[error(transparent)]
    Package(#[from] PluginPackageError),
    #[error("failed to access plugin files: {0}")]
    Io(String),
    #[error("installed plugin manifest is invalid: {0}")]
    InstalledManifest(String),
    #[error("staged plugin failed runtime validation: {0}")]
    RuntimeValidation(String),
    #[error("plugin installation commit failed: {0}")]
    Commit(String),
}

pub(crate) fn install_verified_package(
    package: &VerifiedPluginPackage,
    plugin_root: &Path,
) -> Result<InstallOutcome, PluginInstallationError> {
    install_with_hooks(
        package,
        plugin_root,
        |root| {
            load_official_ui_plugin(root)
                .map(|plugin| Some(plugin.entry_path()))
                .map_err(|error| error.to_string())
        },
        || Ok(()),
    )
}

pub(crate) fn uninstall_plugin(
    plugin_root: &Path,
    install_directory: &str,
) -> Result<UninstallOutcome, PluginInstallationError> {
    let target = plugin_root.join(install_directory);
    if !target.is_dir() {
        return Ok(UninstallOutcome {
            install_directory: install_directory.into(),
            restart_required: false,
        });
    }
    let removing_root = plugin_root.join(REMOVING_DIRECTORY);
    fs::create_dir_all(&removing_root).map_err(io_error)?;
    let removing = removing_root.join(format!("{install_directory}-{}", Uuid::now_v7()));
    fs::rename(&target, &removing).map_err(io_error)?;
    if let Err(error) = fs::remove_dir_all(&removing) {
        if !target.exists() {
            let _ = fs::rename(&removing, &target);
        }
        return Err(io_error(error));
    }
    Ok(UninstallOutcome {
        install_directory: install_directory.into(),
        restart_required: false,
    })
}

pub(crate) fn cleanup_abandoned_staging(plugin_root: &Path) -> Result<(), PluginInstallationError> {
    let staging_root = plugin_root.join(STAGING_DIRECTORY);
    let Ok(entries) = fs::read_dir(&staging_root) else {
        return Ok(());
    };
    for entry in entries {
        let entry = entry.map_err(io_error)?;
        let path = entry.path();
        if path.parent() == Some(staging_root.as_path()) {
            if entry.file_type().map_err(io_error)?.is_dir() {
                fs::remove_dir_all(path).map_err(io_error)?;
            } else {
                fs::remove_file(path).map_err(io_error)?;
            }
        }
    }
    Ok(())
}

fn install_with_hooks(
    package: &VerifiedPluginPackage,
    plugin_root: &Path,
    validate_runtime: impl FnOnce(&Path) -> Result<Option<std::path::PathBuf>, String>,
    before_commit: impl FnOnce() -> Result<(), String>,
) -> Result<InstallOutcome, PluginInstallationError> {
    fs::create_dir_all(plugin_root).map_err(io_error)?;
    cleanup_abandoned_staging(plugin_root)?;

    let target = plugin_root.join(&package.manifest.install_directory);
    let previous_version = installed_version(&target)?;
    validate_compatibility(&package.manifest, previous_version.as_deref())?;

    let transaction = plugin_root
        .join(STAGING_DIRECTORY)
        .join(Uuid::now_v7().to_string());
    fs::create_dir_all(&transaction).map_err(io_error)?;
    let staged_plugin = transaction.join(&package.manifest.install_directory);

    let prepared = (|| {
        extract_payload(package, &transaction)?;
        let entry =
            validate_runtime(&staged_plugin).map_err(PluginInstallationError::RuntimeValidation)?;
        if let Some(entry) = entry {
            set_entry_executable(&entry)?;
        }
        Ok::<(), PluginInstallationError>(())
    })();
    if let Err(error) = prepared {
        let _ = fs::remove_dir_all(&transaction);
        return Err(error);
    }

    let backup_root = plugin_root.join(BACKUP_DIRECTORY);
    fs::create_dir_all(&backup_root).map_err(io_error)?;
    let backup = backup_root.join(format!(
        "{}-{}",
        package.manifest.install_directory,
        Uuid::now_v7()
    ));
    let had_target = target.exists();
    if had_target {
        fs::rename(&target, &backup).map_err(io_error)?;
    }

    let commit = before_commit()
        .map_err(PluginInstallationError::Commit)
        .and_then(|_| fs::rename(&staged_plugin, &target).map_err(io_error));
    if let Err(error) = commit {
        if had_target && !target.exists() {
            let _ = fs::rename(&backup, &target);
        }
        let _ = fs::remove_dir_all(&transaction);
        return Err(error);
    }

    let _ = fs::remove_dir_all(&transaction);
    if had_target {
        fs::remove_dir_all(&backup).map_err(io_error)?;
    }
    Ok(InstallOutcome {
        plugin_id: package.manifest.plugin_id.clone(),
        previous_version,
        installed_version: package.manifest.version.clone(),
        restart_required: true,
    })
}

fn extract_payload(
    package: &VerifiedPluginPackage,
    transaction: &Path,
) -> Result<(), PluginInstallationError> {
    let file = File::open(&package.archive_path).map_err(io_error)?;
    let mut archive =
        ZipArchive::new(file).map_err(|error| PluginInstallationError::Io(error.to_string()))?;
    for declared in &package.manifest.files {
        let mut entry = archive
            .by_name(&declared.path)
            .map_err(|_| PluginPackageError::MissingPayload(declared.path.clone()))?;
        let destination = transaction.join(&declared.path);
        if !destination.starts_with(transaction) {
            return Err(PluginPackageError::UnsafePath(declared.path.clone()).into());
        }
        let parent = destination
            .parent()
            .ok_or_else(|| PluginPackageError::UnsafePath(declared.path.clone()))?;
        fs::create_dir_all(parent).map_err(io_error)?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .map_err(io_error)?;
        let digest = copy_and_hash(&mut entry, &mut output, declared.size, &declared.path)?;
        if digest != declared.sha256 {
            return Err(PluginPackageError::HashMismatch(declared.path.clone()).into());
        }
    }
    Ok(())
}

fn copy_and_hash(
    reader: &mut impl Read,
    writer: &mut impl Write,
    expected_size: u64,
    path: &str,
) -> Result<String, PluginInstallationError> {
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; COPY_BUFFER_BYTES];
    let mut total = 0_u64;
    loop {
        let read = reader.read(&mut buffer).map_err(io_error)?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if total > expected_size {
            return Err(PluginPackageError::SizeMismatch(path.into()).into());
        }
        writer.write_all(&buffer[..read]).map_err(io_error)?;
        hasher.update(&buffer[..read]);
    }
    if total != expected_size {
        return Err(PluginPackageError::SizeMismatch(path.into()).into());
    }
    writer.flush().map_err(io_error)?;
    Ok(hex::encode(hasher.finalize()))
}

fn installed_version(root: &Path) -> Result<Option<String>, PluginInstallationError> {
    let manifest_path = root.join("lumia.plugin.json");
    if !manifest_path.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(manifest_path).map_err(io_error)?;
    let manifest: PluginManifest = serde_json::from_slice(&bytes)
        .map_err(|error| PluginInstallationError::InstalledManifest(error.to_string()))?;
    Ok(Some(manifest.version))
}

#[cfg(unix)]
fn set_entry_executable(entry: &Path) -> Result<(), PluginInstallationError> {
    use std::os::unix::fs::PermissionsExt as _;

    let mut permissions = fs::metadata(entry).map_err(io_error)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(entry, permissions).map_err(io_error)
}

#[cfg(not(unix))]
fn set_entry_executable(_: &Path) -> Result<(), PluginInstallationError> {
    Ok(())
}

fn io_error(error: std::io::Error) -> PluginInstallationError {
    PluginInstallationError::Io(error.to_string())
}

#[cfg(test)]
mod tests;
