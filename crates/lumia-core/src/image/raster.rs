use std::path::Path;

use super::{decode_heic, DecodedImage, ImageLoadError};

pub fn decoded_image_from_rgba(
    mut rgba: Vec<u8>,
    width: u32,
    height: u32,
) -> Result<DecodedImage, ImageLoadError> {
    let expected_len = pixel_buffer_len(width, height)?;
    if rgba.len() != expected_len {
        return Err(invalid_buffer_length("RGBA"));
    }
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Ok(DecodedImage {
        pixels_bgra8: rgba,
        width,
        height,
    })
}

pub fn load_decoded_image_from_path(
    path: impl AsRef<Path>,
) -> Result<DecodedImage, ImageLoadError> {
    let path = path.as_ref();
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .ok_or_else(|| ImageLoadError::MissingExtension(path.to_path_buf()))?;

    if extension.eq_ignore_ascii_case("heic") || extension.eq_ignore_ascii_case("heif") {
        let file_bytes = std::fs::read(path).map_err(|source| ImageLoadError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        return decode_heic(&file_bytes);
    }

    let reader = image::ImageReader::open(path).map_err(|source| ImageLoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let reader = reader
        .with_guessed_format()
        .map_err(|source| ImageLoadError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    let image = reader.decode().map_err(|source| ImageLoadError::Metadata {
        path: path.to_path_buf(),
        source,
    })?;
    let rgba = image.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    decoded_image_from_rgba(rgba.into_raw(), width, height)
}

pub fn rotate_decoded_image(
    decoded: &DecodedImage,
    quarter_turns: u8,
) -> Result<DecodedImage, ImageLoadError> {
    rotate_bgra8(
        &decoded.pixels_bgra8,
        decoded.width,
        decoded.height,
        quarter_turns,
    )
}

pub fn rotate_bgra8(
    source_bgra8: &[u8],
    source_width: u32,
    source_height: u32,
    quarter_turns: u8,
) -> Result<DecodedImage, ImageLoadError> {
    match quarter_turns % 4 {
        0 => Ok(DecodedImage {
            pixels_bgra8: source_bgra8.to_vec(),
            width: source_width,
            height: source_height,
        }),
        turns => {
            let source_len = pixel_buffer_len(source_width, source_height)?;
            if source_bgra8.len() != source_len {
                return Err(invalid_buffer_length("BGRA"));
            }

            let (width, height) = if turns % 2 == 1 {
                (source_height, source_width)
            } else {
                (source_width, source_height)
            };
            let mut pixels_bgra8 = vec![0; source_len];
            for source_y in 0..source_height {
                for source_x in 0..source_width {
                    let (target_x, target_y) = match turns {
                        1 => (source_height - 1 - source_y, source_x),
                        2 => (source_width - 1 - source_x, source_height - 1 - source_y),
                        3 => (source_y, source_width - 1 - source_x),
                        _ => unreachable!(),
                    };
                    let source_offset = ((source_y * source_width + source_x) * 4) as usize;
                    let target_offset = ((target_y * width + target_x) * 4) as usize;
                    pixels_bgra8[target_offset..target_offset + 4]
                        .copy_from_slice(&source_bgra8[source_offset..source_offset + 4]);
                }
            }
            Ok(DecodedImage {
                pixels_bgra8,
                width,
                height,
            })
        }
    }
}

fn pixel_buffer_len(width: u32, height: u32) -> Result<usize, ImageLoadError> {
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| ImageLoadError::HeifMetadata {
            path: "(memory)".into(),
            message: "pixel buffer size overflow".into(),
        })
}

fn invalid_buffer_length(layout: &str) -> ImageLoadError {
    ImageLoadError::HeifMetadata {
        path: "(memory)".into(),
        message: format!("{layout} pixel buffer has an unexpected length"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rgba_conversion_swaps_red_and_blue() {
        let decoded = decoded_image_from_rgba(vec![1, 2, 3, 4], 1, 1).unwrap();
        assert_eq!(decoded.pixels_bgra8, [3, 2, 1, 4]);
    }

    #[test]
    fn rotation_preserves_bgra_pixels_and_dimensions() {
        let decoded = DecodedImage {
            pixels_bgra8: vec![1, 0, 0, 255, 2, 0, 0, 255],
            width: 2,
            height: 1,
        };
        let clockwise = rotate_decoded_image(&decoded, 1).unwrap();
        assert_eq!((clockwise.width, clockwise.height), (1, 2));
        assert_eq!(clockwise.pixels_bgra8, decoded.pixels_bgra8);

        let reversed = rotate_decoded_image(&decoded, 2).unwrap();
        assert_eq!(reversed.pixels_bgra8, [2, 0, 0, 255, 1, 0, 0, 255]);
    }
}
