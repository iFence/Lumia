use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Pre-encoded BMP data ready to pass to GPUI's image element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedImage {
    pub cached_data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageDocument {
    pub id: Uuid,
    pub source: ImageSource,
    pub metadata: Option<ImageMetadata>,
    #[serde(skip)]
    pub cached_image: Option<CachedImage>,
    /// Compatibility bridge used by the asynchronous HEIF decoder.
    #[serde(skip)]
    pub heif_bytes: Option<Vec<u8>>,
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
