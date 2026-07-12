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
            >= 3,
        "Windows, macOS, and Linux releases must build or copy the plugin"
    );
    assert!(release.contains("plugins/lumia-plugin-photoshop/lumia.plugin.json"));

    let wix = std::fs::read_to_string(workspace.join("crates/lumia-app/wix/main.wxs"))
        .expect("WiX source");
    assert!(wix.contains("lumia-plugin-photoshop.exe"));
    assert!(wix.contains("lumia.plugin.json"));

    let installer =
        std::fs::read_to_string(workspace.join("crates/lumia-app/resources/install.sh"))
            .expect("Linux installer");
    assert!(installer.contains("PLUGIN_INSTALL_DIR"));
    assert!(installer.contains("lumia.plugin.json"));

    let plist = std::fs::read_to_string(workspace.join("crates/lumia-app/resources/Info.plist"))
        .expect("macOS bundle metadata");
    assert!(plist.contains("com.adobe.photoshop-image"));
    assert!(plist.contains("com.ifence.lumia.photoshop-large-document"));
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
