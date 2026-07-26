use std::path::{Path, PathBuf};

use uuid::Uuid;

use super::{
    is_supported_image_extension, requires_plugin_preview_extension, ColorDescription,
    ImageDocument, ImageFileMetadata, ImageLoadError, ImageMetadata, ImageProbe, ImageSource,
    PixelFormat, TransferFunction,
};

impl ImageDocument {
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Self {
            id: Uuid::now_v7(),
            source: ImageSource::LocalPath(path.into()),
            metadata: None,
            heif_bytes: None,
        }
    }

    pub fn load_from_path(path: impl AsRef<Path>) -> Result<Self, ImageLoadError> {
        Self::probe_from_path(path).map(|probe| probe.document)
    }

    pub fn probe_from_path(path: impl AsRef<Path>) -> Result<ImageProbe, ImageLoadError> {
        let path = path.as_ref();
        let file_metadata = std::fs::metadata(path).map_err(|source| {
            if source.kind() == std::io::ErrorKind::NotFound {
                ImageLoadError::NotFound(path.to_path_buf())
            } else {
                ImageLoadError::Io {
                    path: path.to_path_buf(),
                    source,
                }
            }
        })?;
        if !file_metadata.is_file() {
            return Err(ImageLoadError::NotAFile(path.to_path_buf()));
        }

        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .ok_or_else(|| ImageLoadError::MissingExtension(path.to_path_buf()))?;
        if !is_supported_image_extension(extension) {
            return Err(ImageLoadError::UnsupportedExtension(extension.to_owned()));
        }

        let (metadata, heif_bytes) = if extension.eq_ignore_ascii_case("svg")
            || requires_plugin_preview_extension(extension)
        {
            (None, None)
        } else if extension.eq_ignore_ascii_case("heic") || extension.eq_ignore_ascii_case("heif") {
            let file_bytes = std::fs::read(path).map_err(|source| ImageLoadError::Io {
                path: path.to_path_buf(),
                source,
            })?;
            let info = heic::ImageInfo::from_bytes(&file_bytes).map_err(|error| {
                ImageLoadError::HeifMetadata {
                    path: path.to_path_buf(),
                    message: error.to_string(),
                }
            })?;

            (
                Some(ImageMetadata {
                    width: info.width,
                    height: info.height,
                    color: ColorDescription {
                        pixel_format: PixelFormat::Unknown,
                        transfer: TransferFunction::Unknown,
                        has_alpha: info.has_alpha,
                    },
                    format_name: Some("HEIF".into()),
                }),
                Some(file_bytes),
            )
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

            (
                Some(ImageMetadata {
                    width,
                    height,
                    color: ColorDescription {
                        pixel_format: PixelFormat::Unknown,
                        transfer: TransferFunction::Unknown,
                        has_alpha: false,
                    },
                    format_name,
                }),
                None,
            )
        };

        Ok(ImageProbe {
            document: Self {
                id: Uuid::now_v7(),
                source: ImageSource::LocalPath(path.to_path_buf()),
                metadata,
                heif_bytes,
            },
            file: ImageFileMetadata {
                size_bytes: file_metadata.len(),
                modified: file_metadata.modified().ok(),
            },
        })
    }
}
