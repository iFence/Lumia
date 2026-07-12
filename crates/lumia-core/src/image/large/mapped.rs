use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
};

use image::{ColorType, ImageDecoder};
use memmap2::MmapMut;

use super::cache::{RasterCacheWriter, RasterLayout};
use super::{cache::RasterCacheKey, decode::bounded_dimensions, LargeImageError};
use crate::{DecodeCancellation, DecodedImage};

const MAX_MAPPED_DECODE_BYTES: u64 = 32 * 1024 * 1024 * 1024;

pub(super) fn decode_mapped_preview(
    path: &Path,
    max_width: u32,
    max_height: u32,
    cache_dir: &Path,
    cancellation: &DecodeCancellation,
) -> Result<DecodedImage, LargeImageError> {
    fs::create_dir_all(cache_dir)?;
    let mut reader = image::ImageReader::open(path)?.with_guessed_format()?;
    reader.no_limits();
    let decoder = reader.into_decoder()?;
    let (width, height) = decoder.dimensions();
    let color_type = decoder.color_type();
    let total_bytes = decoder.total_bytes();
    if total_bytes > MAX_MAPPED_DECODE_BYTES {
        return Err(LargeImageError::MappedImageTooLarge {
            bytes: total_bytes,
            limit: MAX_MAPPED_DECODE_BYTES,
        });
    }
    let mapped_len = usize::try_from(total_bytes).map_err(|_| LargeImageError::SizeOverflow)?;
    let key = RasterCacheKey::from_source(path)?;
    let mapped_path = cache_dir.join(format!("{}.native.part", key.as_str()));
    let _cleanup = RemoveOnDrop(mapped_path.clone());
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&mapped_path)?;
    file.set_len(total_bytes)?;
    // SAFETY: the file remains open and fixed at `total_bytes` for the mapping
    // lifetime. The decoder receives exactly the length it declared.
    let mut map = unsafe { MmapMut::map_mut(&file)? };
    decoder.read_image(&mut map)?;
    if cancellation.is_cancelled() {
        return Err(LargeImageError::Cancelled);
    }
    if map.len() != mapped_len {
        return Err(LargeImageError::InvalidPixelData);
    }

    let (preview_width, preview_height) = bounded_dimensions(width, height, max_width, max_height)?;
    let preview_len = usize::try_from(
        u64::from(preview_width)
            .checked_mul(u64::from(preview_height))
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or(LargeImageError::SizeOverflow)?,
    )
    .map_err(|_| LargeImageError::SizeOverflow)?;
    let mut pixels = vec![0_u8; preview_len];
    let bytes_per_pixel = usize::from(color_type.bytes_per_pixel());

    for output_y in 0..preview_height {
        if cancellation.is_cancelled() {
            return Err(LargeImageError::Cancelled);
        }
        let source_y = sampled_coordinate(output_y, height, preview_height);
        for output_x in 0..preview_width {
            let source_x = sampled_coordinate(output_x, width, preview_width);
            let source_offset = pixel_offset(source_x, source_y, width, bytes_per_pixel)?;
            let output_offset = pixel_offset(output_x, output_y, preview_width, 4)?;
            let pixel = mapped_pixel(&map, source_offset, color_type)?;
            pixels[output_offset..output_offset + 4].copy_from_slice(&pixel);
        }
    }
    Ok(DecodedImage {
        pixels_bgra8: pixels,
        width: preview_width,
        height: preview_height,
    })
}

fn pixel_offset(
    x: u32,
    y: u32,
    width: u32,
    bytes_per_pixel: usize,
) -> Result<usize, LargeImageError> {
    usize::try_from(
        u64::from(y)
            .checked_mul(u64::from(width))
            .and_then(|pixel| pixel.checked_add(u64::from(x)))
            .and_then(|pixel| pixel.checked_mul(bytes_per_pixel as u64))
            .ok_or(LargeImageError::SizeOverflow)?,
    )
    .map_err(|_| LargeImageError::SizeOverflow)
}

fn mapped_pixel(
    bytes: &[u8],
    offset: usize,
    color_type: ColorType,
) -> Result<[u8; 4], LargeImageError> {
    let required = usize::from(color_type.bytes_per_pixel());
    let pixel = bytes
        .get(offset..offset + required)
        .ok_or(LargeImageError::InvalidPixelData)?;
    Ok(match color_type {
        ColorType::L8 => [pixel[0], pixel[0], pixel[0], 255],
        ColorType::La8 => [pixel[0], pixel[0], pixel[0], pixel[1]],
        ColorType::Rgb8 => [pixel[2], pixel[1], pixel[0], 255],
        ColorType::Rgba8 => [pixel[2], pixel[1], pixel[0], pixel[3]],
        ColorType::L16 => {
            let value = u16_to_u8(pixel, 0);
            [value, value, value, 255]
        }
        ColorType::La16 => {
            let value = u16_to_u8(pixel, 0);
            [value, value, value, u16_to_u8(pixel, 2)]
        }
        ColorType::Rgb16 => [
            u16_to_u8(pixel, 4),
            u16_to_u8(pixel, 2),
            u16_to_u8(pixel, 0),
            255,
        ],
        ColorType::Rgba16 => [
            u16_to_u8(pixel, 4),
            u16_to_u8(pixel, 2),
            u16_to_u8(pixel, 0),
            u16_to_u8(pixel, 6),
        ],
        ColorType::Rgb32F => [
            f32_to_u8(pixel, 8),
            f32_to_u8(pixel, 4),
            f32_to_u8(pixel, 0),
            255,
        ],
        ColorType::Rgba32F => [
            f32_to_u8(pixel, 8),
            f32_to_u8(pixel, 4),
            f32_to_u8(pixel, 0),
            f32_to_u8(pixel, 12),
        ],
        _ => return Err(LargeImageError::UnsupportedColorType),
    })
}

pub(super) fn write_mapped_bgra_raster(
    path: &Path,
    cache_dir: &Path,
    key: &str,
    layout: RasterLayout,
    cancellation: &DecodeCancellation,
) -> Result<PathBuf, LargeImageError> {
    let mut reader = image::ImageReader::open(path)?.with_guessed_format()?;
    reader.no_limits();
    let decoder = reader.into_decoder()?;
    let (width, height) = decoder.dimensions();
    if (width, height) != (layout.width(), layout.height()) {
        return Err(LargeImageError::InvalidPixelData);
    }
    let color_type = decoder.color_type();
    let total_bytes = decoder.total_bytes();
    if total_bytes > MAX_MAPPED_DECODE_BYTES {
        return Err(LargeImageError::MappedImageTooLarge {
            bytes: total_bytes,
            limit: MAX_MAPPED_DECODE_BYTES,
        });
    }
    let native_path = cache_dir.join(format!("{key}.native.part"));
    let _cleanup = RemoveOnDrop(native_path.clone());
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(native_path)?;
    file.set_len(total_bytes)?;
    // SAFETY: the fixed-size file outlives the mapping and the decoder writes
    // exactly its declared byte count.
    let mut map = unsafe { MmapMut::map_mut(&file)? };
    decoder.read_image(&mut map)?;
    if cancellation.is_cancelled() {
        return Err(LargeImageError::Cancelled);
    }

    let source_stride = usize::try_from(
        u64::from(width)
            .checked_mul(u64::from(color_type.bytes_per_pixel()))
            .ok_or(LargeImageError::SizeOverflow)?,
    )
    .map_err(|_| LargeImageError::SizeOverflow)?;
    let bytes_per_pixel = usize::from(color_type.bytes_per_pixel());
    let mut writer = RasterCacheWriter::create(cache_dir, key, layout)?;
    for y in 0..height {
        if cancellation.is_cancelled() {
            return Err(LargeImageError::Cancelled);
        }
        let source_row = usize::try_from(y)
            .ok()
            .and_then(|row| row.checked_mul(source_stride))
            .ok_or(LargeImageError::SizeOverflow)?;
        let output = writer.row_mut(y).ok_or(LargeImageError::InvalidPixelData)?;
        for x in 0..width {
            let source = source_row
                .checked_add(
                    usize::try_from(x)
                        .ok()
                        .and_then(|value| value.checked_mul(bytes_per_pixel))
                        .ok_or(LargeImageError::SizeOverflow)?,
                )
                .ok_or(LargeImageError::SizeOverflow)?;
            let destination = usize::try_from(x)
                .ok()
                .and_then(|value| value.checked_mul(4))
                .ok_or(LargeImageError::SizeOverflow)?;
            let pixel = mapped_pixel(&map, source, color_type)?;
            output[destination..destination + 4].copy_from_slice(&pixel);
        }
    }
    writer.finish()
}

fn u16_to_u8(bytes: &[u8], offset: usize) -> u8 {
    (u16::from_ne_bytes([bytes[offset], bytes[offset + 1]]) >> 8) as u8
}

fn f32_to_u8(bytes: &[u8], offset: usize) -> u8 {
    let value = f32::from_ne_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]);
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn sampled_coordinate(output: u32, source_size: u32, output_size: u32) -> u32 {
    ((u64::from(output) * u64::from(source_size)) / u64::from(output_size)) as u32
}

struct RemoveOnDrop(PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
