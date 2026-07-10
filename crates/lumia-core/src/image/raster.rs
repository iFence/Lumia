use std::path::Path;

use image::ImageEncoder;

use super::{decode_heic_to_png, CachedImage, ImageLoadError};

pub fn cached_image_from_rgba(
    rgba: &[u8],
    width: u32,
    height: u32,
) -> Result<CachedImage, ImageLoadError> {
    let mut cached_data = Vec::new();
    image::codecs::bmp::BmpEncoder::new(&mut cached_data)
        .write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
        .map_err(|error| ImageLoadError::Io {
            path: "(memory)".into(),
            source: std::io::Error::other(error.to_string()),
        })?;

    Ok(CachedImage {
        cached_data,
        width,
        height,
    })
}

pub fn load_cached_image_from_path(path: impl AsRef<Path>) -> Result<CachedImage, ImageLoadError> {
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
        return decode_heic_to_png(&file_bytes);
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
    cached_image_from_rgba(rgba.as_raw(), rgba.width(), rgba.height())
}

pub fn rotate_cached_image(
    cached: &CachedImage,
    quarter_turns: u8,
) -> Result<CachedImage, ImageLoadError> {
    match quarter_turns % 4 {
        0 => Ok(cached.clone()),
        turns => {
            let image =
                image::load_from_memory_with_format(&cached.cached_data, image::ImageFormat::Bmp)
                    .map_err(|source| ImageLoadError::Metadata {
                        path: "(memory)".into(),
                        source,
                    })?
                    .to_rgba8();
            let rotated = match turns {
                1 => image::imageops::rotate90(&image),
                2 => image::imageops::rotate180(&image),
                3 => image::imageops::rotate270(&image),
                _ => unreachable!(),
            };
            cached_image_from_rgba(rotated.as_raw(), rotated.width(), rotated.height())
        }
    }
}
