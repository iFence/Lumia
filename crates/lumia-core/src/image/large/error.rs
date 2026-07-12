use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum LargeImageError {
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
    #[error("large image cache I/O failed")]
    Io(#[from] io::Error),
}
