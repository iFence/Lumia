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
    let source_width = u32::from(info.width);
    let source_height = u32::from(info.height);
    let (request_width, request_height) =
        jpeg_scale_request(source_width, source_height, max_width, max_height)?;
    let (scaled_width, scaled_height) = decoder.scale(request_width, request_height)?;
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
    validate_output_len(
        &pixels_bgra8,
        u32::from(scaled_width),
        u32::from(scaled_height),
    )?;
    Ok(Some(DecodedImage {
        pixels_bgra8,
        width: u32::from(scaled_width),
        height: u32::from(scaled_height),
    }))
}

/// `jpeg-decoder` exposes only five DCT output sizes (1, 1/2, 1/4, and 1/8 of
/// each edge) and interprets the requested size as the minimum output size.
/// Pass a request just above the next smaller output so the decoder selects
/// the largest output that fits the caller's preview bounds.
fn jpeg_scale_request(
    width: u32,
    height: u32,
    max_width: u32,
    max_height: u32,
) -> Result<(u16, u16), LargeImageError> {
    let bounded = bounded_dimensions(width, height, max_width, max_height)?;
    let mut selected = None;
    for divisor in [1_u32, 2, 4, 8] {
        let scaled = jpeg_output_dimensions(width, height, divisor);
        if scaled.0 <= bounded.0 && scaled.1 <= bounded.1 {
            selected = Some(divisor);
            break;
        }
    }
    let divisor = selected.ok_or(LargeImageError::SizeOverflow)?;
    let previous = divisor.saturating_mul(2);
    let (previous_width, previous_height) = jpeg_output_dimensions(width, height, previous);
    let request_width = previous_width.saturating_add(1);
    let request_height = previous_height.saturating_add(1);
    Ok((
        u16::try_from(request_width).map_err(|_| LargeImageError::SizeOverflow)?,
        u16::try_from(request_height).map_err(|_| LargeImageError::SizeOverflow)?,
    ))
}

fn jpeg_output_dimensions(width: u32, height: u32, divisor: u32) -> (u32, u32) {
    (div_ceil(width, divisor), div_ceil(height, divisor))
}

fn div_ceil(value: u32, divisor: u32) -> u32 {
    value / divisor + u32::from(value % divisor != 0)
}

fn l8_to_bgra(input: &[u8]) -> Result<Vec<u8>, LargeImageError> {
    let capacity = input
        .len()
        .checked_mul(4)
        .ok_or(LargeImageError::SizeOverflow)?;
    let mut output = vec![0_u8; capacity];
    for (&value, pixel) in input.iter().zip(output.chunks_exact_mut(4)) {
        pixel.copy_from_slice(&[value, value, value, 255]);
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
    let mut output = vec![0_u8; capacity];
    for (source, destination) in input.chunks_exact(3).zip(output.chunks_exact_mut(4)) {
        destination.copy_from_slice(&[source[2], source[1], source[0], 255]);
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
    let mut output = vec![0_u8; capacity];
    for (source, destination) in input.chunks_exact(4).zip(output.chunks_exact_mut(4)) {
        let key = 255_u16.saturating_sub(u16::from(source[3]));
        let red = ((255_u16.saturating_sub(u16::from(source[0]))) * key / 255) as u8;
        let green = ((255_u16.saturating_sub(u16::from(source[1]))) * key / 255) as u8;
        let blue = ((255_u16.saturating_sub(u16::from(source[2]))) * key / 255) as u8;
        destination.copy_from_slice(&[blue, green, red, 255]);
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

    #[test]
    fn scale_request_selects_the_largest_supported_size_within_bounds() {
        assert_eq!(
            jpeg_scale_request(4032, 3024, 2048, 2048).unwrap(),
            (1009, 757)
        );
        assert_eq!(jpeg_scale_request(64, 32, 16, 16).unwrap(), (9, 5));
        assert_eq!(jpeg_output_dimensions(4032, 3024, 2), (2016, 1512));
    }
}
