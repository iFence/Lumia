use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, SystemTime};
use uuid::Uuid;

/// Decoded pixels in BGRA8 byte order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedImage {
    pub pixels_bgra8: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodePolicy {
    pub max_output_bytes: u64,
    pub max_alloc_bytes: u64,
}

impl Default for DecodePolicy {
    fn default() -> Self {
        Self {
            max_output_bytes: 96 * 1024 * 1024,
            max_alloc_bytes: 192 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageFileMetadata {
    pub size_bytes: u64,
    pub modified: Option<SystemTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageProbe {
    pub document: ImageDocument,
    pub file: ImageFileMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedAnimationFrame {
    pub image: DecodedImage,
    pub delay: Duration,
}

#[derive(Debug, Clone, Default)]
pub struct DecodeCancellation {
    cancelled: Arc<AtomicBool>,
}

impl DecodeCancellation {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

impl heic::Stop for DecodeCancellation {
    fn check(&self) -> Result<(), heic::StopReason> {
        if self.is_cancelled() {
            Err(heic::StopReason::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageDocument {
    pub id: Uuid,
    pub source: ImageSource,
    pub metadata: Option<ImageMetadata>,
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
    #[serde(default)]
    pub exif: ExifMetadata,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExifMetadata {
    pub chroma_subsampling: Option<String>,
    pub color_space: Option<String>,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub lens: Option<String>,
    pub software: Option<String>,
    pub date_taken: Option<String>,
    pub flash: Option<String>,
    pub focal_length: Option<String>,
    pub exposure_time: Option<String>,
    pub exposure_bias: Option<String>,
    pub aperture: Option<String>,
    pub iso: Option<String>,
    pub exposure_program: Option<String>,
    pub metering_mode: Option<String>,
    pub gps: Option<String>,
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
