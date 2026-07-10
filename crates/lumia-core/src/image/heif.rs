use super::{cached_image_from_rgba, CachedImage, ImageLoadError};

/// Decode HEIC/HEIF bytes into the BMP-backed cache used by the viewer.
///
/// This compatibility bridge is intentionally isolated so it can later move
/// behind the official plugin boundary without affecting the document model.
pub fn decode_heic_to_png(file_bytes: &[u8]) -> Result<CachedImage, ImageLoadError> {
    let info =
        heic::ImageInfo::from_bytes(file_bytes).map_err(|error| ImageLoadError::HeifMetadata {
            path: "(memory)".into(),
            message: error.to_string(),
        })?;
    let pixel_count = (info.width as usize)
        .checked_mul(info.height as usize)
        .ok_or_else(|| ImageLoadError::HeifMetadata {
            path: "(memory)".into(),
            message: "image dimensions overflow".into(),
        })?;
    let buffer_size = pixel_count
        .checked_mul(4)
        .ok_or_else(|| ImageLoadError::HeifMetadata {
            path: "(memory)".into(),
            message: "pixel buffer size overflow".into(),
        })?;

    let mut rgba = vec![0; buffer_size];
    heic::DecoderConfig::default()
        .decode_request(file_bytes)
        .decode_into(&mut rgba)
        .map_err(|error| ImageLoadError::HeifMetadata {
            path: "(memory)".into(),
            message: error.to_string(),
        })?;
    cached_image_from_rgba(&rgba, info.width, info.height)
}
