use super::*;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

const ONE_BY_ONE_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

fn temp_path(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("lumia-core-test-{nonce}-{name}"))
}

#[test]
fn supported_extension_matching_is_case_insensitive() {
    assert!(is_supported_image_extension("png"));
    assert!(is_supported_image_extension("PNG"));
    assert!(is_supported_image_extension("JpEg"));
    assert!(!is_supported_image_extension("txt"));
}

#[test]
fn load_from_path_reports_missing_directory_and_unsupported_extension() {
    let missing = temp_path("missing.png");
    assert!(matches!(
        ImageDocument::load_from_path(&missing),
        Err(ImageLoadError::NotFound(_))
    ));

    let dir = temp_path("dir");
    fs::create_dir(&dir).expect("create temp dir");
    assert!(matches!(
        ImageDocument::load_from_path(&dir),
        Err(ImageLoadError::NotAFile(_))
    ));
    fs::remove_dir(&dir).expect("remove temp dir");

    let text_file = temp_path("note.txt");
    fs::write(&text_file, b"not an image").expect("write temp text file");
    assert!(matches!(
        ImageDocument::load_from_path(&text_file),
        Err(ImageLoadError::UnsupportedExtension(extension)) if extension == "txt"
    ));
    fs::remove_file(&text_file).expect("remove temp text file");
}

#[test]
fn load_from_path_reads_raster_metadata_and_allows_svg_without_metadata() {
    let png = temp_path("image.PNG");
    fs::write(&png, ONE_BY_ONE_PNG).expect("write temp png");
    let document = ImageDocument::load_from_path(&png).expect("load png");
    let metadata = document.metadata.expect("png metadata");
    assert_eq!(metadata.width, 1);
    assert_eq!(metadata.height, 1);
    assert_eq!(metadata.format_name.as_deref(), Some("Png"));
    fs::remove_file(&png).expect("remove temp png");

    let svg = temp_path("image.svg");
    fs::write(&svg, "<svg xmlns=\"http://www.w3.org/2000/svg\"/>").expect("write temp svg");
    let document = ImageDocument::load_from_path(&svg).expect("load svg");
    assert!(document.metadata.is_none());
    fs::remove_file(&svg).expect("remove temp svg");
}

#[test]
fn load_from_path_accepts_plugin_preview_documents_without_core_decode() {
    for extension in ["psd", "PSB"] {
        let path = temp_path(&format!("image.{extension}"));
        fs::write(&path, b"plugin-owned image data").expect("write plugin image");
        let document = ImageDocument::load_from_path(&path).expect("accept plugin image path");
        assert!(document.metadata.is_none());
        fs::remove_file(path).expect("remove plugin image");
    }
}

#[test]
fn probe_records_file_size_with_image_metadata() {
    let png = temp_path("probe.png");
    fs::write(&png, ONE_BY_ONE_PNG).expect("write temp png");

    let probe = ImageDocument::probe_from_path(&png).expect("probe png");
    assert_eq!(probe.file.size_bytes, ONE_BY_ONE_PNG.len() as u64);
    assert_eq!(
        probe
            .document
            .metadata
            .as_ref()
            .map(|metadata| (metadata.width, metadata.height)),
        Some((1, 1))
    );
    fs::remove_file(png).expect("remove temp png");
}
