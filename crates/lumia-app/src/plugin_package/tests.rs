use std::fs::File;
use std::io::Write;

use base64::Engine as _;
use ring::signature::{Ed25519KeyPair, KeyPair};
use sha2::{Digest, Sha256};
use tempfile::TempDir;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use super::*;

const TEST_SEED: [u8; 32] = [7; 32];
const PAYLOAD_PATH: &str = "lumia-plugin-annotation/plugin.bin";
const PAYLOAD: &[u8] = b"official annotation executable";

#[test]
fn package_manifest_round_trips_schema_and_compatibility() {
    let manifest = compatible_manifest(vec![declared_file(PAYLOAD_PATH, PAYLOAD)]);
    let json = serde_json::to_string(&manifest).unwrap();
    let decoded: PluginPackageManifest = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, manifest);
    validate_compatibility(&manifest, None).unwrap();
}

/// Regression test for the allowlist relaxation: any plugin id passes
/// compatibility now, as long as the package signature and structural checks
/// verify (signature verification is the trust gate, not an id allowlist).
#[test]
fn third_party_plugin_id_passes_compatibility() {
    let manifest = compatible_manifest(Vec::new());
    assert_eq!(manifest.plugin_id, "lumia.annotation"); // fixture sanity
    let mut third_party = manifest;
    third_party.plugin_id = "com.example.foo".into();
    third_party.install_directory = "foo".into();
    validate_compatibility(&third_party, None).unwrap();
}

#[test]
fn compatibility_rejects_wrong_schema_platform_api_and_downgrade() {
    let mut manifest = compatible_manifest(Vec::new());
    manifest.schema_version = 2;
    assert_eq!(
        validate_compatibility(&manifest, None).unwrap_err(),
        PluginPackageError::UnsupportedSchema(2)
    );

    let mut manifest = compatible_manifest(Vec::new());
    manifest.target_os = "other".into();
    assert!(matches!(
        validate_compatibility(&manifest, None),
        Err(PluginPackageError::IncompatiblePlatform { .. })
    ));

    let mut manifest = compatible_manifest(Vec::new());
    manifest.plugin_api_version += 1;
    assert!(matches!(
        validate_compatibility(&manifest, None),
        Err(PluginPackageError::IncompatiblePluginApi { .. })
    ));

    let manifest = compatible_manifest(Vec::new());
    assert_eq!(
        validate_compatibility(&manifest, Some("0.2.0")).unwrap_err(),
        PluginPackageError::DowngradeBlocked {
            installed: "0.2.0".into(),
            package: "0.1.0".into(),
        }
    );
}

#[test]
fn valid_signed_package_is_inspected_without_extraction() {
    let fixture = PackageFixture::valid();
    let verified =
        inspect_package_with_key(&fixture.path, None, fixture.key_pair.public_key().as_ref())
            .unwrap();

    assert_eq!(verified.archive_path, fixture.path);
    assert_eq!(verified.manifest.plugin_id, "lumia.annotation");
}

#[test]
fn changed_manifest_and_tampered_payload_are_rejected() {
    let changed_manifest = PackageFixture::build(PackageOptions {
        mutate_manifest_after_signing: true,
        ..PackageOptions::default()
    });
    assert_eq!(
        inspect_package_with_key(
            &changed_manifest.path,
            None,
            changed_manifest.key_pair.public_key().as_ref()
        )
        .unwrap_err(),
        PluginPackageError::InvalidSignature
    );

    let tampered = PackageFixture::build(PackageOptions {
        payload: b"tampered executable".to_vec(),
        declared_payload: PAYLOAD.to_vec(),
        ..PackageOptions::default()
    });
    assert_eq!(
        inspect_package_with_key(
            &tampered.path,
            None,
            tampered.key_pair.public_key().as_ref()
        )
        .unwrap_err(),
        PluginPackageError::SizeMismatch(PAYLOAD_PATH.into())
    );
}

#[test]
fn missing_extra_duplicate_and_case_colliding_files_are_rejected() {
    let missing = PackageFixture::build(PackageOptions {
        omit_payload: true,
        ..PackageOptions::default()
    });
    assert_eq!(
        inspect_package_with_key(&missing.path, None, missing.key_pair.public_key().as_ref())
            .unwrap_err(),
        PluginPackageError::MissingPayload(PAYLOAD_PATH.into())
    );

    let extra = PackageFixture::build(PackageOptions {
        extra_files: vec![("lumia-plugin-annotation/extra.bin".into(), vec![1])],
        ..PackageOptions::default()
    });
    assert_eq!(
        inspect_package_with_key(&extra.path, None, extra.key_pair.public_key().as_ref())
            .unwrap_err(),
        PluginPackageError::UnexpectedPayload("lumia-plugin-annotation/extra.bin".into())
    );

    let duplicate_manifest = compatible_manifest(vec![
        declared_file(PAYLOAD_PATH, PAYLOAD),
        declared_file(PAYLOAD_PATH, PAYLOAD),
    ]);
    assert!(matches!(
        validate_declarations(&duplicate_manifest),
        Err(PluginPackageError::DuplicatePath(_))
    ));

    let collision = PackageFixture::build(PackageOptions {
        extra_files: vec![(
            "LUMIA-PLUGIN-ANNOTATION/PLUGIN.BIN".into(),
            PAYLOAD.to_vec(),
        )],
        ..PackageOptions::default()
    });
    assert!(matches!(
        inspect_package_with_key(
            &collision.path,
            None,
            collision.key_pair.public_key().as_ref()
        ),
        Err(PluginPackageError::DuplicatePath(_))
    ));
}

#[test]
fn traversal_absolute_reserved_and_symlink_entries_are_rejected() {
    for unsafe_path in [
        "../escape.bin",
        "/absolute.bin",
        "lumia-plugin-annotation/CON",
        "lumia-plugin-annotation/trailing.",
        r"lumia-plugin-annotation\backslash.bin",
    ] {
        let fixture = PackageFixture::build(PackageOptions {
            extra_files: vec![(unsafe_path.into(), vec![1])],
            ..PackageOptions::default()
        });
        assert!(matches!(
            inspect_package_with_key(&fixture.path, None, fixture.key_pair.public_key().as_ref()),
            Err(PluginPackageError::UnsafePath(_))
        ));
    }

    let symlink = PackageFixture::build(PackageOptions {
        symlink: Some((
            "lumia-plugin-annotation/link".into(),
            "../../outside".into(),
        )),
        ..PackageOptions::default()
    });
    assert!(matches!(
        inspect_package_with_key(&symlink.path, None, symlink.key_pair.public_key().as_ref()),
        Err(PluginPackageError::UnsupportedEntryType(_))
    ));
}

#[test]
fn declared_file_and_payload_limits_are_enforced() {
    let too_large = compatible_manifest(vec![PluginPackageFile {
        path: PAYLOAD_PATH.into(),
        size: MAX_FILE_BYTES + 1,
        sha256: "a".repeat(64),
    }]);
    assert_eq!(
        validate_declarations(&too_large).unwrap_err(),
        PluginPackageError::FileTooLarge(PAYLOAD_PATH.into())
    );

    let many_files = (0..=MAX_PAYLOAD_FILES)
        .map(|index| PluginPackageFile {
            path: format!("lumia-plugin-annotation/{index}.bin"),
            size: 0,
            sha256: hex::encode(Sha256::digest([])),
        })
        .collect();
    assert_eq!(
        validate_declarations(&compatible_manifest(many_files)).unwrap_err(),
        PluginPackageError::TooManyPayloadFiles
    );
}

struct PackageFixture {
    _directory: TempDir,
    path: PathBuf,
    key_pair: Ed25519KeyPair,
}

#[derive(Default)]
struct PackageOptions {
    payload: Vec<u8>,
    declared_payload: Vec<u8>,
    omit_payload: bool,
    mutate_manifest_after_signing: bool,
    extra_files: Vec<(String, Vec<u8>)>,
    symlink: Option<(String, String)>,
}

impl PackageFixture {
    fn valid() -> Self {
        Self::build(PackageOptions::default())
    }

    fn build(mut options: PackageOptions) -> Self {
        if options.payload.is_empty() {
            options.payload = PAYLOAD.to_vec();
        }
        if options.declared_payload.is_empty() {
            options.declared_payload = options.payload.clone();
        }
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("fixture.lumiaplugin");
        let key_pair = Ed25519KeyPair::from_seed_unchecked(&TEST_SEED).unwrap();
        let manifest =
            compatible_manifest(vec![declared_file(PAYLOAD_PATH, &options.declared_payload)]);
        let signed_manifest = serde_json::to_vec(&manifest).unwrap();
        let signature = base64::engine::general_purpose::STANDARD
            .encode(key_pair.sign(&signed_manifest).as_ref());
        let mut archive_manifest = signed_manifest;
        if options.mutate_manifest_after_signing {
            archive_manifest.push(b' ');
        }

        let file = File::create(&path).unwrap();
        let mut writer = ZipWriter::new(file);
        let file_options =
            SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        write_file(
            &mut writer,
            PACKAGE_MANIFEST_NAME,
            &archive_manifest,
            file_options,
        );
        write_file(
            &mut writer,
            PACKAGE_SIGNATURE_NAME,
            signature.as_bytes(),
            file_options,
        );
        if !options.omit_payload {
            write_file(&mut writer, PAYLOAD_PATH, &options.payload, file_options);
        }
        for (name, bytes) in options.extra_files {
            write_file(&mut writer, &name, &bytes, file_options);
        }
        if let Some((name, target)) = options.symlink {
            writer
                .add_symlink(name, target, SimpleFileOptions::default())
                .unwrap();
        }
        writer.finish().unwrap();

        Self {
            _directory: directory,
            path,
            key_pair,
        }
    }
}

fn write_file(writer: &mut ZipWriter<File>, path: &str, bytes: &[u8], options: SimpleFileOptions) {
    writer.start_file(path, options).unwrap();
    writer.write_all(bytes).unwrap();
}

fn compatible_manifest(files: Vec<PluginPackageFile>) -> PluginPackageManifest {
    PluginPackageManifest {
        schema_version: 1,
        plugin_id: "lumia.annotation".into(),
        version: "0.1.0".into(),
        plugin_api_version: lumia_plugin_api::PROTOCOL_VERSION,
        minimum_lumia_version: env!("CARGO_PKG_VERSION").into(),
        target_os: current_target_os().into(),
        target_arch: current_target_arch().into(),
        install_directory: "lumia-plugin-annotation".into(),
        files,
    }
}

fn declared_file(path: &str, bytes: &[u8]) -> PluginPackageFile {
    PluginPackageFile {
        path: path.into(),
        size: bytes.len() as u64,
        sha256: hex::encode(Sha256::digest(bytes)),
    }
}
