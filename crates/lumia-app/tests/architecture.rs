use std::path::{Path, PathBuf};

const MAX_PRODUCTION_LINES: usize = 500;

#[test]
fn production_rust_modules_stay_below_hard_size_limit() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("lumia-app must be under workspace/crates");
    let mut rust_files = Vec::new();
    collect_rust_files(&workspace.join("crates"), &mut rust_files);
    collect_rust_files(&workspace.join("plugins"), &mut rust_files);

    let offenders = rust_files
        .into_iter()
        .filter_map(|path| {
            let source = std::fs::read_to_string(&path).ok()?;
            let lines = source.lines().count();
            (lines > MAX_PRODUCTION_LINES).then_some((path, lines))
        })
        .collect::<Vec<_>>();

    assert!(
        offenders.is_empty(),
        "production modules exceed {MAX_PRODUCTION_LINES} lines: {offenders:#?}"
    );
}

#[test]
fn official_photoshop_plugin_is_declared_in_every_release_package() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("lumia-app must be under workspace/crates");
    let release = std::fs::read_to_string(workspace.join(".github/workflows/release.yml"))
        .expect("release workflow");
    assert!(
        release
            .matches("target/release/lumia-plugin-photoshop")
            .count()
            >= 2,
        "Windows and Linux releases must copy the plugin"
    );
    assert!(release.contains("plugins/lumia-plugin-photoshop/lumia.plugin.json"));
    let macos_installer =
        std::fs::read_to_string(workspace.join("scripts/build-macos-installer.sh"))
            .expect("macOS installer");
    assert!(macos_installer.contains("target/release/lumia-plugin-photoshop"));
    assert!(macos_installer.contains("plugins/lumia-plugin-photoshop/lumia.plugin.json"));

    let wix = std::fs::read_to_string(workspace.join("crates/lumia-app/wix/main.wxs"))
        .expect("WiX source");
    assert!(wix.contains("lumia-plugin-photoshop.exe"));
    assert!(wix.contains("lumia.plugin.json"));

    let installer =
        std::fs::read_to_string(workspace.join("crates/lumia-app/resources/install.sh"))
            .expect("Linux installer");
    assert!(installer.contains("PLUGIN_INSTALL_DIR"));
    assert!(installer.contains("lumia.plugin.json"));
    assert!(installer.contains("lumia-image-formats.xml"));

    let plist = std::fs::read_to_string(workspace.join("crates/lumia-app/resources/Info.plist"))
        .expect("macOS bundle metadata");
    assert!(plist.contains("com.adobe.photoshop-image"));
    assert!(plist.contains("com.aspirin.lumia.photoshop-large-document"));
}

#[test]
fn official_modern_format_plugins_are_declared_in_every_release_package() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("lumia-app must be under workspace/crates");
    let release = std::fs::read_to_string(workspace.join(".github/workflows/release.yml"))
        .expect("release workflow");
    let macos_installer =
        std::fs::read_to_string(workspace.join("scripts/build-macos-installer.sh"))
            .expect("macOS installer");
    let wix = std::fs::read_to_string(workspace.join("crates/lumia-app/wix/main.wxs"))
        .expect("WiX source");
    let linux_installer =
        std::fs::read_to_string(workspace.join("crates/lumia-app/resources/install.sh"))
            .expect("Linux installer");

    for plugin in ["lumia-plugin-jpeg-xl", "lumia-plugin-jpeg2000"] {
        assert!(release.contains(&format!("target/release/{plugin}")));
        assert!(release.contains(&format!("plugins/{plugin}/lumia.plugin.json")));
        assert!(macos_installer.contains(&format!("target/release/{plugin}")));
        assert!(macos_installer.contains(&format!("plugins/{plugin}/lumia.plugin.json")));
        assert!(wix.contains(&format!("{plugin}.exe")));
        assert!(linux_installer.contains(plugin));
    }
}

#[test]
fn annotation_plugin_is_packaged_separately_with_integrity_metadata() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("lumia-app must be under workspace/crates");
    let release = std::fs::read_to_string(workspace.join(".github/workflows/release.yml"))
        .expect("release workflow");
    for artifact in [
        "Lumia-Annotation-windows-x64.lumiaplugin",
        "Lumia-Annotation-macos-${{ matrix.arch }}.lumiaplugin",
        "Lumia-Annotation-linux-x64.lumiaplugin",
    ] {
        assert!(release.contains(artifact), "missing {artifact}");
    }
    assert!(
        release.contains("--verify-plugin-package"),
        "release must verify final plugin packages with Lumia"
    );

    let unix_packager =
        std::fs::read_to_string(workspace.join("scripts/package-annotation-plugin.sh"))
            .expect("Unix Annotation packager");
    let windows_packager =
        std::fs::read_to_string(workspace.join("scripts/package-annotation-plugin.ps1"))
            .expect("Windows Annotation packager");
    for required in [
        "lumia.plugin.json",
        "lumia.plugin.sig",
        "lumia.package.json",
        "lumia.package.sig",
        "assets",
    ] {
        assert!(
            unix_packager.contains(required),
            "Unix package misses {required}"
        );
        assert!(
            windows_packager.contains(required),
            "Windows package misses {required}"
        );
    }

    let wix = std::fs::read_to_string(workspace.join("crates/lumia-app/wix/main.wxs"))
        .expect("WiX source");
    assert!(
        !wix.contains("lumia-plugin-annotation"),
        "optional Annotation plugin must not enter the base MSI"
    );
    let macos_installer =
        std::fs::read_to_string(workspace.join("scripts/build-macos-installer.sh"))
            .expect("macOS installer");
    assert!(
        !macos_installer.contains("lumia-plugin-annotation"),
        "optional Annotation plugin must not enter the base app bundle"
    );
}

#[test]
fn raw_plugin_is_pinned_packaged_separately_and_signed() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("lumia-app must be under workspace/crates");
    let release = std::fs::read_to_string(workspace.join(".github/workflows/release.yml"))
        .expect("release workflow");
    for artifact in [
        "Lumia-RAW-windows-x64.lumiaplugin",
        "Lumia-RAW-macos-${{ matrix.arch }}.lumiaplugin",
        "Lumia-RAW-linux-x64.lumiaplugin",
    ] {
        assert!(release.contains(artifact), "missing {artifact}");
    }

    assert_eq!(
        release
            .matches("cc0_raw_samples_decode_to_bounded_png")
            .count(),
        3,
        "all release platforms must decode the fixed RAW samples"
    );
    let samples = std::fs::read_to_string(workspace.join("scripts/fetch-raw-test-samples.mjs"))
        .expect("RAW sample fetcher");
    for required in ["CC0", "dng", "cr3", "nef", "arw", "raf"] {
        assert!(samples.contains(required), "RAW samples miss {required}");
    }
    assert_eq!(samples.matches("sha256:").count(), 5);
    let native_windows = std::fs::read_to_string(workspace.join("scripts/build-raw-native.ps1"))
        .expect("Windows RAW native builder");
    let native_unix = std::fs::read_to_string(workspace.join("scripts/build-raw-native.sh"))
        .expect("Unix RAW native builder");
    for required in [
        "b93f6e45c194f5df9b02a43b1af9a54b4f41f33f",
        "plugins/lumia-plugin-raw/native",
        "ENABLE_X3FTOOLS",
    ] {
        assert!(
            native_windows.contains(required),
            "Windows builder misses {required}"
        );
        assert!(
            native_unix.contains(required),
            "Unix builder misses {required}"
        );
    }

    let signer = std::fs::read_to_string(workspace.join("scripts/sign-plugin-package.mjs"))
        .expect("plugin signer");
    assert!(signer.contains("lumia.raw"));
    assert!(signer.contains("lumia.plugin.sig"));

    let wix = std::fs::read_to_string(workspace.join("crates/lumia-app/wix/main.wxs"))
        .expect("WiX source");
    assert!(
        !wix.contains("lumia-plugin-raw"),
        "optional RAW plugin must not enter the base MSI"
    );
    let macos_installer =
        std::fs::read_to_string(workspace.join("scripts/build-macos-installer.sh"))
            .expect("macOS installer");
    assert!(
        !macos_installer.contains("lumia-plugin-raw"),
        "optional RAW plugin must not enter the base app bundle"
    );
}
#[test]
fn macos_release_builds_native_arm64_and_x64_packages() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("lumia-app must be under workspace/crates");
    let release = std::fs::read_to_string(workspace.join(".github/workflows/release.yml"))
        .expect("release workflow");

    for required in [
        "runner: macos-15",
        "runner: macos-15-intel",
        "arch: arm64",
        "arch: x64",
        "Lumia-macos-${{ matrix.arch }}.dmg",
        "Lumia-Annotation-macos-${{ matrix.arch }}.lumiaplugin",
    ] {
        assert!(release.contains(required), "missing {required}");
    }
}

#[test]
fn release_metadata_declares_file_association_resources() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("lumia-app must be under workspace/crates");
    let release = std::fs::read_to_string(workspace.join(".github/workflows/release.yml"))
        .expect("release workflow");
    assert!(release.contains("resources/lumia.desktop"));
    assert!(release.contains("resources/lumia-mime.xml"));

    let plist = std::fs::read_to_string(workspace.join("crates/lumia-app/resources/Info.plist"))
        .expect("macOS bundle metadata");
    for content_type in [
        "public.jpeg",
        "public.png",
        "public.heic",
        "com.aspirin.lumia.dds",
        "com.aspirin.lumia.photoshop-large-document",
        "com.aspirin.lumia.camera-raw",
    ] {
        assert!(plist.contains(content_type), "missing {content_type}");
    }
}

fn collect_rust_files(root: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs")
            && path
                .components()
                .any(|component| component.as_os_str() == "src")
        {
            files.push(path);
        }
    }
}
