use super::{DecodeCancellation, DecodedImage, ImageLoadError};

/// This compatibility bridge is intentionally isolated so it can later move
/// behind the official plugin boundary without affecting the document model.
pub fn decode_heic(file_bytes: &[u8]) -> Result<DecodedImage, ImageLoadError> {
    decode_heic_with_cancellation(file_bytes, &DecodeCancellation::default())
}

pub fn decode_heic_with_cancellation(
    file_bytes: &[u8],
    cancellation: &DecodeCancellation,
) -> Result<DecodedImage, ImageLoadError> {
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

    let mut pixels_bgra8 = vec![0; buffer_size];
    heic::DecoderConfig::default()
        .decode_request(file_bytes)
        .with_output_layout(heic::PixelLayout::Bgra8)
        .with_stop(cancellation)
        .decode_into(&mut pixels_bgra8)
        .map_err(|error| ImageLoadError::HeifMetadata {
            path: "(memory)".into(),
            message: error.to_string(),
        })?;
    Ok(DecodedImage {
        pixels_bgra8,
        width: info.width,
        height: info.height,
    })
}

pub fn decode_heic_thumbnail(file_bytes: &[u8]) -> Result<Option<DecodedImage>, ImageLoadError> {
    heic::DecoderConfig::default()
        .decode_thumbnail(file_bytes, heic::PixelLayout::Bgra8)
        .map(|thumbnail| {
            thumbnail.map(|thumbnail| DecodedImage {
                pixels_bgra8: thumbnail.data,
                width: thumbnail.width,
                height: thumbnail.height,
            })
        })
        .map_err(|error| ImageLoadError::HeifMetadata {
            path: "(memory)".into(),
            message: error.to_string(),
        })
}
