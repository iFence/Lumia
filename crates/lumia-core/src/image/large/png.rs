use std::{fs::File, io::BufReader, path::Path};

use png::{BitDepth, ColorType, Decoder, Transformations};

use super::{decode::bounded_dimensions, LargeImageError};
use crate::{DecodeCancellation, DecodedImage};

pub(super) fn decode_png_preview(
    path: &Path,
    max_width: u32,
    max_height: u32,
    cancellation: &DecodeCancellation,
) -> Result<Option<DecodedImage>, LargeImageError> {
    let mut decoder = Decoder::new(BufReader::new(File::open(path)?));
    decoder.set_transformations(Transformations::EXPAND | Transformations::STRIP_16);
    let mut reader = decoder.read_info()?;
    let info = reader.info();
    if info.interlaced {
        return Ok(None);
    }
    let width = info.width;
    let height = info.height;
    let (preview_width, preview_height) = bounded_dimensions(width, height, max_width, max_height)?;
    let (color_type, bit_depth) = reader.output_color_type();
    if bit_depth != BitDepth::Eight {
        return Err(LargeImageError::InvalidPixelData);
    }
    let channels = png_channels(color_type);
    let preview_len = usize::try_from(
        u64::from(preview_width)
            .checked_mul(u64::from(preview_height))
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(LargeImageError::SizeOverflow)?,
    )
    .map_err(|_| LargeImageError::SizeOverflow)?;
    let mut pixels = vec![0_u8; preview_len];
    let mut preview_y = 0_u32;

    for source_y in 0..height {
        if cancellation.is_cancelled() {
            return Err(LargeImageError::Cancelled);
        }
        let row = reader
            .next_row()?
            .ok_or(LargeImageError::InvalidPixelData)?;
        while preview_y < preview_height
            && sampled_coordinate(preview_y, height, preview_height) == source_y
        {
            copy_sampled_row(
                row.data(),
                color_type,
                channels,
                width,
                &mut pixels,
                preview_width,
                preview_y,
            )?;
            preview_y += 1;
        }
    }
    if preview_y != preview_height {
        return Err(LargeImageError::InvalidPixelData);
    }
    Ok(Some(DecodedImage {
        pixels_bgra8: pixels,
        width: preview_width,
        height: preview_height,
    }))
}

fn copy_sampled_row(
    row: &[u8],
    color_type: ColorType,
    channels: usize,
    source_width: u32,
    output: &mut [u8],
    output_width: u32,
    output_y: u32,
) -> Result<(), LargeImageError> {
    let expected = usize::try_from(source_width)
        .ok()
        .and_then(|width| width.checked_mul(channels))
        .ok_or(LargeImageError::SizeOverflow)?;
    if row.len() < expected {
        return Err(LargeImageError::InvalidPixelData);
    }
    for output_x in 0..output_width {
        let source_x = sampled_coordinate(output_x, source_width, output_width);
        let source_offset = usize::try_from(source_x)
            .ok()
            .and_then(|x| x.checked_mul(channels))
            .ok_or(LargeImageError::SizeOverflow)?;
        let output_offset = (usize::try_from(output_y)
            .ok()
            .and_then(|y| y.checked_mul(usize::try_from(output_width).ok()?))
            .and_then(|pixel| pixel.checked_add(usize::try_from(output_x).ok()?))
            .and_then(|pixel| pixel.checked_mul(4)))
        .ok_or(LargeImageError::SizeOverflow)?;
        let pixel = png_pixel(row, source_offset, color_type);
        output[output_offset..output_offset + 4].copy_from_slice(&pixel);
    }
    Ok(())
}

pub(super) fn png_pixel(row: &[u8], offset: usize, color_type: ColorType) -> [u8; 4] {
    match color_type {
        ColorType::Grayscale => {
            let value = row[offset];
            [value, value, value, 255]
        }
        ColorType::GrayscaleAlpha => {
            let value = row[offset];
            [value, value, value, row[offset + 1]]
        }
        ColorType::Rgb => [row[offset + 2], row[offset + 1], row[offset], 255],
        ColorType::Rgba => [
            row[offset + 2],
            row[offset + 1],
            row[offset],
            row[offset + 3],
        ],
        ColorType::Indexed => unreachable!("EXPAND removes indexed PNG output"),
    }
}

pub(super) const fn png_channels(color_type: ColorType) -> usize {
    match color_type {
        ColorType::Grayscale => 1,
        ColorType::GrayscaleAlpha => 2,
        ColorType::Rgb => 3,
        ColorType::Rgba => 4,
        ColorType::Indexed => 1,
    }
}

fn sampled_coordinate(output: u32, source_size: u32, output_size: u32) -> u32 {
    ((u64::from(output) * u64::from(source_size)) / u64::from(output_size)) as u32
}
