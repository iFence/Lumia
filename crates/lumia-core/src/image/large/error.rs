use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LargeImageError {
    #[error("large image preview bounds must be greater than zero")]
    InvalidBounds,
    #[error("image dimensions must be greater than zero")]
    InvalidDimensions,
    #[error("image dimensions exceed addressable storage")]
    SizeOverflow,
    #[error("large image cache key is invalid")]
    InvalidCacheKey,
    #[error("large image cache has length {actual}, expected {expected}")]
    InvalidCacheLength { expected: u64, actual: u64 },
    #[error(
        "large image cache requires {required} bytes but only {available} bytes are available"
    )]
    InsufficientDiskSpace { required: u64, available: u64 },
    #[error("large image decode was cancelled")]
    Cancelled,
    #[error("decoded image requires {bytes} bytes, exceeding the {limit}-byte mapped limit")]
    MappedImageTooLarge { bytes: u64, limit: u64 },
    #[error("large image decoder returned an incomplete pixel row")]
    InvalidPixelData,
    #[error("large image uses an unsupported decoded color type")]
    UnsupportedColorType,
    #[error("large image decode failed")]
    Image(#[from] image::ImageError),
    #[error("large PNG decode failed")]
    Png(#[from] png::DecodingError),
    #[error("large image cache I/O failed")]
    Io(#[from] io::Error),
}
