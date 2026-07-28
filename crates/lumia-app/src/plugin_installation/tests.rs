use std::fs::File;
use std::io::Write;
use std::path::Path;

use sha2::{Digest, Sha256};
use tempfile::TempDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use super::*;
use crate::plugin_package::{
    current_target_arch, current_target_os, PluginPackageFile, PluginPackageManifest,
};

const INSTALL_DIRECTORY: &str = "lumia-plugin-annotation";
const PAYLOAD_PATH: &str = "lumia-plugin-annotation/plugin.bin";

#[test]
fn installs_reinstalls_and_upgrades_atomically() {
    let root = tempfile::tempdir().unwrap();
    let package = package_fixture("0.1.0", b"first");
    let outcome = install_for_test(&package.package, root.path()).unwrap();
    assert_eq!(outcome.previous_version, None);
    assert_eq!(fs::read(root.path().join(PAYLOAD_PATH)).unwrap(), b"first");

    write_installed_manifest(root.path(), "0.1.0");
    let replacement = package_fixture("0.1.0", b"replacement");
    let outcome = install_for_test(&replacement.package, root.path()).unwrap();
    assert_eq!(outcome.previous_version.as_deref(), Some("0.1.0"));
    assert_eq!(
        fs::read(root.path().join(PAYLOAD_PATH)).unwrap(),
        b"replacement"
    );

    write_installed_manifest(root.path(), "0.1.0");
    let upgrade = package_fixture("0.2.0", b"upgrade");
    let outcome = install_for_test(&upgrade.package, root.path()).unwrap();
    assert_eq!(outcome.installed_version, "0.2.0");
    assert_eq!(
        fs::read(root.path().join(PAYLOAD_PATH)).unwrap(),
        b"upgrade"
    );
}

#[test]
fn downgrade_is_rejected_without_changing_installed_files() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join(INSTALL_DIRECTORY);
    fs::create_dir_all(&target).unwrap();
    write_installed_manifest(root.path(), "0.2.0");
    fs::write(target.join("marker"), b"current").unwrap();

    let downgrade = package_fixture("0.1.0", b"old");
    let error = install_for_test(&downgrade.package, root.path()).unwrap_err();
    assert!(matches!(
        error,
        PluginInstallationError::Package(PluginPackageError::DowngradeBlocked { .. })
    ));
    assert_eq!(fs::read(target.join("marker")).unwrap(), b"current");
}

#[test]
fn commit_failure_restores_previous_version() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join(INSTALL_DIRECTORY);
    fs::create_dir_all(&target).unwrap();
    write_installed_manifest(root.path(), "0.1.0");
    fs::write(target.join("marker"), b"current").unwrap();
    let update = package_fixture("0.2.0", b"new");

    let error = install_with_hooks(
        &update.package,
        root.path(),
        |_| Ok(None),
        || Err("injected commit failure".into()),
    )
    .unwrap_err();

    assert!(matches!(error, PluginInstallationError::Commit(_)));
    assert_eq!(fs::read(target.join("marker")).unwrap(), b"current");
}

#[test]
fn abandoned_staging_is_cleaned_before_install() {
    let root = tempfile::tempdir().unwrap();
    let abandoned = root.path().join(STAGING_DIRECTORY).join("abandoned");
    fs::create_dir_all(&abandoned).unwrap();
    fs::write(abandoned.join("partial"), b"partial").unwrap();

    let package = package_fixture("0.1.0", b"installed");
    install_for_test(&package.package, root.path()).unwrap();

    assert!(!abandoned.exists());
    assert!(root.path().join(PAYLOAD_PATH).is_file());
}

#[test]
fn uninstall_removes_only_the_exact_plugin_directory() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join(INSTALL_DIRECTORY);
    fs::create_dir_all(&target).unwrap();
    fs::write(target.join("plugin.bin"), b"plugin").unwrap();
    fs::write(root.path().join("keep"), b"keep").unwrap();

    let outcome = uninstall_plugin(root.path(), INSTALL_DIRECTORY).unwrap();
    assert_eq!(outcome.install_directory, INSTALL_DIRECTORY);
    assert!(!target.exists());
    assert_eq!(fs::read(root.path().join("keep")).unwrap(), b"keep");
}

fn install_for_test(
    package: &VerifiedPluginPackage,
    root: &Path,
) -> Result<InstallOutcome, PluginInstallationError> {
    install_with_hooks(package, root, |_| Ok(None), || Ok(()))
}

struct PackageFixture {
    _directory: TempDir,
    package: VerifiedPluginPackage,
}

fn package_fixture(version: &str, payload: &[u8]) -> PackageFixture {
    let directory = tempfile::tempdir().unwrap();
    let archive_path = directory.path().join("package.lumiaplugin");
    let manifest = PluginPackageManifest {
        schema_version: 1,
        plugin_id: "lumia.annotation".into(),
        version: version.into(),
        plugin_api_version: lumia_plugin_api::PROTOCOL_VERSION,
        minimum_lumia_version: env!("CARGO_PKG_VERSION").into(),
        target_os: current_target_os().into(),
        target_arch: current_target_arch().into(),
        install_directory: INSTALL_DIRECTORY.into(),
        files: vec![PluginPackageFile {
            path: PAYLOAD_PATH.into(),
            size: payload.len() as u64,
            sha256: hex::encode(Sha256::digest(payload)),
        }],
    };
    let file = File::create(&archive_path).unwrap();
    let mut writer = ZipWriter::new(file);
    writer
        .start_file(
            PAYLOAD_PATH,
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
        )
        .unwrap();
    writer.write_all(payload).unwrap();
    writer.finish().unwrap();
    PackageFixture {
        _directory: directory,
        package: VerifiedPluginPackage {
            archive_path,
            manifest,
        },
    }
}

fn write_installed_manifest(plugin_root: &Path, version: &str) {
    let target = plugin_root.join(INSTALL_DIRECTORY);
    fs::create_dir_all(&target).unwrap();
    let manifest = lumia_plugin_api::PluginManifest {
        id: "lumia.annotation".into(),
        name: "Annotation".into(),
        version: version.into(),
        entry: "plugin.bin".into(),
        capabilities: Vec::new(),
        permissions: Vec::new(),
        supported_inputs: Vec::new(),
        supported_outputs: Vec::new(),
        contributions: Default::default(),
        assets: Vec::new(),
    };
    fs::write(
        target.join("lumia.plugin.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();
}
