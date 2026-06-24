use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

pub const SUPPORTED_IMAGE_EXTENSIONS: &[&str] = &[
    "avif", "jpg", "jpeg", "png", "gif", "webp", "tif", "tiff", "tga", "dds", "bmp", "ico", "hdr",
    "exr", "pbm", "pam", "ppm", "pgm", "ff", "farbfeld", "qoi", "svg",
    "heic", "heif",
];

pub fn supported_image_extensions() -> &'static [&'static str] {
    SUPPORTED_IMAGE_EXTENSIONS
}

pub fn is_supported_image_extension(extension: &str) -> bool {
    SUPPORTED_IMAGE_EXTENSIONS
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(extension))
}

#[derive(Debug, Error)]
pub enum ImageLoadError {
    #[error("file does not exist: {0}")]
    NotFound(PathBuf),
    #[error("path is not a file: {0}")]
    NotAFile(PathBuf),
    #[error("image extension is not supported: {0}")]
    UnsupportedExtension(String),
    #[error("image path has no file extension: {0}")]
    MissingExtension(PathBuf),
    #[error("failed to read HEIF metadata from {path}: {message}")]
    HeifMetadata { path: PathBuf, message: String },
    #[error("failed to read image metadata from {path}: {source}")]
    Metadata {
        path: PathBuf,
        #[source]
        source: image::ImageError,
    },
    #[error("failed to open image {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageDocument {
    pub id: Uuid,
    pub source: ImageSource,
    pub metadata: Option<ImageMetadata>,
}

impl ImageDocument {
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self {
            id: Uuid::now_v7(),
            source: ImageSource::LocalPath(path.into()),
            metadata: None,
        }
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, ImageLoadError> {
        let path = path.as_ref();
        if !path.exists() {
            return Err(ImageLoadError::NotFound(path.to_path_buf()));
        }
        if !path.is_file() {
            return Err(ImageLoadError::NotAFile(path.to_path_buf()));
        }

        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .ok_or_else(|| ImageLoadError::MissingExtension(path.to_path_buf()))?;
        if !is_supported_image_extension(extension) {
            return Err(ImageLoadError::UnsupportedExtension(extension.to_owned()));
        }

        let metadata = if extension.eq_ignore_ascii_case("svg") {
            None
        } else if extension.eq_ignore_ascii_case("heic")
            || extension.eq_ignore_ascii_case("heif")
        {
            let file_bytes =
                std::fs::read(path).map_err(|source| ImageLoadError::Io {
                    path: path.to_path_buf(),
                    source,
                })?;
            let info = heic::ImageInfo::from_bytes(&file_bytes).map_err(|err| {
                ImageLoadError::HeifMetadata {
                    path: path.to_path_buf(),
                    message: err.to_string(),
                }
            })?;
            Some(ImageMetadata {
                width: info.width,
                height: info.height,
                color: ColorDescription {
                    pixel_format: PixelFormat::Unknown,
                    transfer: TransferFunction::Unknown,
                    has_alpha: false,
                },
                format_name: Some("HEIF".into()),
            })
        } else {
            let reader = image::ImageReader::open(path).map_err(|source| ImageLoadError::Io {
                path: path.to_path_buf(),
                source,
            })?;
            let reader = reader
                .with_guessed_format()
                .map_err(|source| ImageLoadError::Io {
                    path: path.to_path_buf(),
                    source,
                })?;
            let format_name = reader.format().map(|format| format!("{format:?}"));
            let (width, height) =
                reader
                    .into_dimensions()
                    .map_err(|source| ImageLoadError::Metadata {
                        path: path.to_path_buf(),
                        source,
                    })?;

            Some(ImageMetadata {
                width,
                height,
                color: ColorDescription {
                    pixel_format: PixelFormat::Unknown,
                    transfer: TransferFunction::Unknown,
                    has_alpha: false,
                },
                format_name,
            })
        };

        Ok(Self {
            id: Uuid::now_v7(),
            source: ImageSource::LocalPath(path.to_path_buf()),
            metadata,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageSource {
    LocalPath(PathBuf),
    TemporaryPath(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageMetadata {
    pub width: u32,
    pub height: u32,
    pub color: ColorDescription,
    pub format_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColorDescription {
    pub pixel_format: PixelFormat,
    pub transfer: TransferFunction,
    pub has_alpha: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PixelFormat {
    U8,
    U16,
    F16,
    F32,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferFunction {
    Srgb,
    Linear,
    Hlg,
    Pq,
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    const ONE_BY_ONE_PNG: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
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
}
