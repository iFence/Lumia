use std::path::PathBuf;

use thiserror::Error;

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
