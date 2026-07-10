mod document;
mod error;
mod formats;
mod heif;
mod raster;
mod types;

pub use error::ImageLoadError;
pub use formats::{
    is_supported_image_extension, supported_image_extensions, SUPPORTED_IMAGE_EXTENSIONS,
};
pub use heif::decode_heic_to_png;
pub use raster::{cached_image_from_rgba, load_cached_image_from_path, rotate_cached_image};
pub use types::{
    CachedImage, ColorDescription, ImageDocument, ImageMetadata, ImageSource, PixelFormat,
    TransferFunction,
};

#[cfg(test)]
mod tests;
