use std::{fs::File, io::BufReader, path::Path};

use jpeg_decoder::PixelFormat;

use super::{decode::bounded_dimensions, LargeImageError};
use crate::{DecodeCancellation, DecodedImage};

pub(super) fn decode_jpeg_preview(
    path: &Path,
    max_width: u32,
    max_height: u32,
    cancellation: &DecodeCancellation,
) -> Result<Option<DecodedImage>, LargeImageError> {
    if cancellation.is_cancelled() {
        return Err(LargeImageError::Cancelled);
    }

    let mut decoder = jpeg_decoder::Decoder::new(BufReader::new(File::open(path)?));
    decoder.read_info()?;
    let info = decoder.info().ok_or(LargeImageError::InvalidPixelData)?;
    let (preview_width, preview_height) = bounded_dimensions(
        u32::from(info.width),
        u32::from(info.height),
        max_width,
        max_height,
    )?;
    let requested_width =
        u16::try_from(preview_width).map_err(|_| LargeImageError::SizeOverflow)?;
    let requested_height =
        u16::try_from(preview_height).map_err(|_| LargeImageError::SizeOverflow)?;
    let (scaled_width, scaled_height) = decoder.scale(requested_width, requested_height)?;
    let max_buffer = usize::from(scaled_width)
        .checked_mul(usize::from(scaled_height))
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(LargeImageError::SizeOverflow)?;
    decoder.set_max_decoding_buffer_size(max_buffer);

    let decoded = decoder.decode()?;
    if cancellation.is_cancelled() {
        return Err(LargeImageError::Cancelled);
    }
    let info = decoder.info().ok_or(LargeImageError::InvalidPixelData)?;
    let pixels_bgra8 = match info.pixel_format {
        PixelFormat::L8 => l8_to_bgra(&decoded)?,
        PixelFormat::RGB24 => rgb_to_bgra(&decoded)?,
        PixelFormat::CMYK32 => cmyk_to_bgra(&decoded)?,
        PixelFormat::L16 => return Ok(None),
    };
    validate_output_len(&pixels_bgra8, u32::from(info.width), u32::from(info.height))?;
    Ok(Some(DecodedImage {
        pixels_bgra8,
        width: u32::from(info.width),
        height: u32::from(info.height),
    }))
}

fn l8_to_bgra(input: &[u8]) -> Result<Vec<u8>, LargeImageError> {
    let capacity = input
        .len()
        .checked_mul(4)
        .ok_or(LargeImageError::SizeOverflow)?;
    let mut output = Vec::with_capacity(capacity);
    for &value in input {
        output.extend_from_slice(&[value, value, value, 255]);
    }
    Ok(output)
}

fn rgb_to_bgra(input: &[u8]) -> Result<Vec<u8>, LargeImageError> {
    let pixels = input
        .len()
        .checked_div(3)
        .filter(|pixels| pixels.saturating_mul(3) == input.len())
        .ok_or(LargeImageError::InvalidPixelData)?;
    let capacity = pixels.checked_mul(4).ok_or(LargeImageError::SizeOverflow)?;
    let mut output = Vec::with_capacity(capacity);
    for pixel in input.chunks_exact(3) {
        output.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
    }
    Ok(output)
}

fn cmyk_to_bgra(input: &[u8]) -> Result<Vec<u8>, LargeImageError> {
    let pixels = input
        .len()
        .checked_div(4)
        .filter(|pixels| pixels.saturating_mul(4) == input.len())
        .ok_or(LargeImageError::InvalidPixelData)?;
    let capacity = pixels.checked_mul(4).ok_or(LargeImageError::SizeOverflow)?;
    let mut output = Vec::with_capacity(capacity);
    for pixel in input.chunks_exact(4) {
        let key = 255_u16.saturating_sub(u16::from(pixel[3]));
        let red = ((255_u16.saturating_sub(u16::from(pixel[0]))) * key / 255) as u8;
        let green = ((255_u16.saturating_sub(u16::from(pixel[1]))) * key / 255) as u8;
        let blue = ((255_u16.saturating_sub(u16::from(pixel[2]))) * key / 255) as u8;
        output.extend_from_slice(&[blue, green, red, 255]);
    }
    Ok(output)
}

fn validate_output_len(pixels: &[u8], width: u32, height: u32) -> Result<(), LargeImageError> {
    let expected = usize::try_from(
        u64::from(width)
            .checked_mul(u64::from(height))
            .and_then(|value| value.checked_mul(4))
            .ok_or(LargeImageError::SizeOverflow)?,
    )
    .map_err(|_| LargeImageError::SizeOverflow)?;
    if pixels.len() != expected {
        return Err(LargeImageError::InvalidPixelData);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_conversions_output_bgra() {
        assert_eq!(l8_to_bgra(&[17]).unwrap(), [17, 17, 17, 255]);
        assert_eq!(rgb_to_bgra(&[10, 20, 30]).unwrap(), [30, 20, 10, 255]);
        assert_eq!(cmyk_to_bgra(&[255, 0, 0, 255]).unwrap(), [0, 0, 0, 255]);
    }

    #[test]
    fn malformed_color_buffers_are_rejected() {
        assert!(matches!(
            rgb_to_bgra(&[1, 2]),
            Err(LargeImageError::InvalidPixelData)
        ));
        assert!(matches!(
            cmyk_to_bgra(&[1, 2, 3]),
            Err(LargeImageError::InvalidPixelData)
        ));
    }
}
