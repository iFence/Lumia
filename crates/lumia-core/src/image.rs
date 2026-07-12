mod document;
mod error;
mod formats;
mod heif;
mod large;
mod raster;
mod types;

pub use error::ImageLoadError;
pub use formats::{
    is_supported_image_extension, requires_plugin_preview_extension, supported_image_extensions,
    supported_image_format_groups, ImageFormatGroup, SUPPORTED_IMAGE_EXTENSIONS,
    SUPPORTED_IMAGE_FORMAT_GROUPS,
};
pub use heif::{decode_heic, decode_heic_thumbnail, decode_heic_with_cancellation};
pub use large::decode_large_image_preview;
pub use large::{
    build_large_image_raster, large_image_worker_count, LargeImageRaster, PixelBudget,
};
pub use large::{checked_bgra8_len, ImagePixelRect, LargeImagePolicy, TileCoordinate, TileLevel};
pub use raster::{
    decoded_image_from_rgba, load_decoded_image_from_path, rotate_bgra8, rotate_decoded_image,
};
pub use types::{
    ColorDescription, DecodeCancellation, DecodedImage, ImageDocument, ImageMetadata, ImageSource,
    PixelFormat, TransferFunction,
};

#[cfg(test)]
mod tests;
