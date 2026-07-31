mod animation;
mod document;
mod edit;
mod error;
mod formats;
mod heif;
mod large;
mod metadata;
mod raster;
mod types;

pub use edit::{
    apply_image_edit, export_decoded_image, CropRect, ImageEditError, ImageEditOperation,
    ImageEditPolicy, ImageExportFormat,
};
pub use error::ImageLoadError;
pub use formats::{
    is_raw_image_extension, is_supported_image_extension, requires_plugin_preview_extension,
    supported_image_extensions, supported_image_format_groups, ImageFormatGroup,
    RAW_IMAGE_EXTENSIONS, SUPPORTED_IMAGE_EXTENSIONS, SUPPORTED_IMAGE_FORMAT_GROUPS,
};
pub use heif::{decode_heic, decode_heic_thumbnail, decode_heic_with_cancellation};
pub use large::decode_large_image_preview;
pub use large::LargeImageError;
pub use large::{
    build_large_image_raster, large_image_worker_count, LargeImageRaster, PixelBudget,
};
pub use large::{checked_bgra8_len, ImagePixelRect, LargeImagePolicy, TileCoordinate, TileLevel};
pub use raster::{
    decoded_image_from_rgba, load_decoded_image_from_path,
    load_decoded_image_from_path_with_policy, rotate_bgra8, rotate_decoded_image,
};
pub use types::{
    ColorDescription, DecodeCancellation, DecodePolicy, DecodedAnimationFrame, DecodedImage,
    ExifMetadata, GpsCoordinates, ImageDocument, ImageFileMetadata, ImageMetadata, ImageProbe,
    ImageSource, PixelFormat, TransferFunction,
};

#[cfg(test)]
mod tests;
pub use animation::stream_gif_frames;
