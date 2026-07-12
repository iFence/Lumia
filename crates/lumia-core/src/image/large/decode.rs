use std::path::Path;

use image::ImageFormat;

use super::{mapped::decode_mapped_preview, png::decode_png_preview, LargeImageError};
use crate::{DecodeCancellation, DecodedImage};

pub fn decode_large_image_preview(
    path: &Path,
    max_width: u32,
    max_height: u32,
    cache_dir: &Path,
    cancellation: &DecodeCancellation,
) -> Result<DecodedImage, LargeImageError> {
    if max_width == 0 || max_height == 0 {
        return Err(LargeImageError::InvalidBounds);
    }
    if cancellation.is_cancelled() {
        return Err(LargeImageError::Cancelled);
    }

    let reader = image::ImageReader::open(path)?.with_guessed_format()?;
    if reader.format() == Some(ImageFormat::Png) {
        if let Some(preview) = decode_png_preview(path, max_width, max_height, cancellation)? {
            return Ok(preview);
        }
    }

    decode_mapped_preview(path, max_width, max_height, cache_dir, cancellation)
}

pub(super) fn bounded_dimensions(
    width: u32,
    height: u32,
    max_width: u32,
    max_height: u32,
) -> Result<(u32, u32), LargeImageError> {
    if width == 0 || height == 0 {
        return Err(LargeImageError::InvalidDimensions);
    }
    if max_width == 0 || max_height == 0 {
        return Err(LargeImageError::InvalidBounds);
    }
    if width <= max_width && height <= max_height {
        return Ok((width, height));
    }
    let scale =
        (f64::from(max_width) / f64::from(width)).min(f64::from(max_height) / f64::from(height));
    Ok((
        ((f64::from(width) * scale).floor() as u32).max(1),
        ((f64::from(height) * scale).floor() as u32).max(1),
    ))
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, time::SystemTime};

    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};

    use super::*;
    fn temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("lumia-large-preview-{nonce}-{name}"))
    }

    fn write_fixture(path: &PathBuf, format: ImageFormat, width: u32, height: u32) {
        let image = RgbaImage::from_fn(width, height, |x, y| {
            Rgba([(x * 11) as u8, (y * 17) as u8, 91, 200])
        });
        DynamicImage::ImageRgba8(image)
            .save_with_format(path, format)
            .unwrap();
    }

    #[test]
    fn common_formats_decode_to_bounded_bgra_previews() {
        let dir = temp_dir("formats");
        let cache = dir.join("cache");
        fs::create_dir_all(&cache).unwrap();
        for (format, extension) in [
            (ImageFormat::Png, "png"),
            (ImageFormat::Jpeg, "jpg"),
            (ImageFormat::WebP, "webp"),
            (ImageFormat::Bmp, "bmp"),
            (ImageFormat::Tiff, "tiff"),
            (ImageFormat::Gif, "gif"),
        ] {
            let path = dir.join(format!("fixture.{extension}"));
            write_fixture(&path, format, 8, 4);
            let preview =
                decode_large_image_preview(&path, 4, 4, &cache, &DecodeCancellation::default())
                    .unwrap_or_else(|error| panic!("{extension} preview failed: {error}"));
            assert_eq!((preview.width, preview.height), (4, 2), "{extension}");
            assert_eq!(preview.pixels_bgra8.len(), 32, "{extension}");
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn png_preserves_alpha_and_outputs_bgra_order() {
        let dir = temp_dir("alpha");
        let cache = dir.join("cache");
        fs::create_dir_all(&cache).unwrap();
        let path = dir.join("alpha.png");
        let image = RgbaImage::from_pixel(1, 1, Rgba([10, 20, 30, 40]));
        DynamicImage::ImageRgba8(image)
            .save_with_format(&path, ImageFormat::Png)
            .unwrap();

        let preview =
            decode_large_image_preview(&path, 10, 10, &cache, &DecodeCancellation::default())
                .unwrap();
        assert_eq!(preview.pixels_bgra8, [30, 20, 10, 40]);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn wide_png_allocates_only_the_bounded_preview() {
        let dir = temp_dir("wide");
        let cache = dir.join("cache");
        fs::create_dir_all(&cache).unwrap();
        let path = dir.join("wide.png");
        write_fixture(&path, ImageFormat::Png, 16384, 2);

        let preview =
            decode_large_image_preview(&path, 64, 64, &cache, &DecodeCancellation::default())
                .unwrap();
        assert_eq!((preview.width, preview.height), (64, 1));
        assert_eq!(preview.pixels_bgra8.len(), 64 * 4);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cancellation_and_corrupt_input_return_stable_errors() {
        let dir = temp_dir("errors");
        let cache = dir.join("cache");
        fs::create_dir_all(&cache).unwrap();
        let path = dir.join("image.png");
        write_fixture(&path, ImageFormat::Png, 2, 2);
        let cancellation = DecodeCancellation::default();
        cancellation.cancel();
        assert!(matches!(
            decode_large_image_preview(&path, 2, 2, &cache, &cancellation),
            Err(LargeImageError::Cancelled)
        ));

        let corrupt = dir.join("corrupt.png");
        fs::write(&corrupt, b"not an image").unwrap();
        assert!(
            decode_large_image_preview(&corrupt, 2, 2, &cache, &DecodeCancellation::default())
                .is_err()
        );
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    #[ignore = "requires LUMIA_LARGE_IMAGE_SAMPLE"]
    fn external_large_image_sample_decodes_to_a_bounded_preview() {
        let path = PathBuf::from(std::env::var_os("LUMIA_LARGE_IMAGE_SAMPLE").unwrap());
        let dir = temp_dir("external");
        let cache = dir.join("cache");
        fs::create_dir_all(&cache).unwrap();
        let preview =
            decode_large_image_preview(&path, 2048, 2048, &cache, &DecodeCancellation::default())
                .unwrap();
        assert!(preview.width <= 2048);
        assert!(preview.height <= 2048);
        assert_eq!(
            preview.pixels_bgra8.len(),
            usize::try_from(u64::from(preview.width) * u64::from(preview.height) * 4).unwrap()
        );
        fs::remove_dir_all(dir).unwrap();
    }
}
