use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::codecs::webp::WebPEncoder;
use image::{ExtendedColorType, ImageEncoder, RgbaImage};
use thiserror::Error;

use super::DecodedImage;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CropRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl CropRect {
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageEditOperation {
    Crop(CropRect),
    Resize { width: u32, height: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageExportFormat {
    Png,
    Jpeg,
    WebP,
}

impl ImageExportFormat {
    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension.to_ascii_lowercase().as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "webp" => Some(Self::WebP),
            _ => None,
        }
    }

    pub const fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::WebP => "webp",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageEditPolicy {
    pub max_output_bytes: u64,
}

impl Default for ImageEditPolicy {
    fn default() -> Self {
        Self {
            max_output_bytes: 96 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Error)]
pub enum ImageEditError {
    #[error("crop rectangle is empty or outside the source image")]
    InvalidCrop,
    #[error("resize dimensions must be greater than zero")]
    InvalidResize,
    #[error("decoded image has an unexpected pixel buffer length")]
    InvalidPixelBuffer,
    #[error("edited image requires {bytes} bytes, exceeding the {limit} byte limit")]
    MemoryLimit { bytes: u64, limit: u64 },
    #[error("failed to create exported image {path}: {source}")]
    Create {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to encode exported image {path}: {source}")]
    Encode {
        path: PathBuf,
        #[source]
        source: image::ImageError,
    },
}

pub fn apply_image_edit(
    source: &DecodedImage,
    operation: ImageEditOperation,
    policy: ImageEditPolicy,
) -> Result<DecodedImage, ImageEditError> {
    validate_source(source)?;
    match operation {
        ImageEditOperation::Crop(rect) => crop_image(source, rect, policy),
        ImageEditOperation::Resize { width, height } => resize_image(source, width, height, policy),
    }
}

pub fn export_decoded_image(
    image: &DecodedImage,
    path: impl AsRef<Path>,
    format: ImageExportFormat,
) -> Result<(), ImageEditError> {
    validate_source(image)?;
    let path = path.as_ref();
    let file = File::create(path).map_err(|source| ImageEditError::Create {
        path: path.to_path_buf(),
        source,
    })?;
    let writer = BufWriter::new(file);
    let rgba = bgra_to_rgba(&image.pixels_bgra8);
    let result = match format {
        ImageExportFormat::Png => PngEncoder::new(writer).write_image(
            &rgba,
            image.width,
            image.height,
            ExtendedColorType::Rgba8,
        ),
        ImageExportFormat::Jpeg => {
            let rgb = flatten_rgba_on_white(&rgba);
            JpegEncoder::new_with_quality(writer, 90).write_image(
                &rgb,
                image.width,
                image.height,
                ExtendedColorType::Rgb8,
            )
        }
        ImageExportFormat::WebP => WebPEncoder::new_lossless(writer).write_image(
            &rgba,
            image.width,
            image.height,
            ExtendedColorType::Rgba8,
        ),
    };
    result.map_err(|source| ImageEditError::Encode {
        path: path.to_path_buf(),
        source,
    })
}

fn crop_image(
    source: &DecodedImage,
    rect: CropRect,
    policy: ImageEditPolicy,
) -> Result<DecodedImage, ImageEditError> {
    let right = rect
        .x
        .checked_add(rect.width)
        .ok_or(ImageEditError::InvalidCrop)?;
    let bottom = rect
        .y
        .checked_add(rect.height)
        .ok_or(ImageEditError::InvalidCrop)?;
    if rect.width == 0 || rect.height == 0 || right > source.width || bottom > source.height {
        return Err(ImageEditError::InvalidCrop);
    }
    let output_len = checked_output_len(rect.width, rect.height, policy)?;
    let mut pixels = Vec::with_capacity(output_len);
    let source_stride = source.width as usize * 4;
    let row_len = rect.width as usize * 4;
    for y in rect.y..bottom {
        let start = y as usize * source_stride + rect.x as usize * 4;
        pixels.extend_from_slice(&source.pixels_bgra8[start..start + row_len]);
    }
    Ok(DecodedImage {
        pixels_bgra8: pixels,
        width: rect.width,
        height: rect.height,
    })
}

fn resize_image(
    source: &DecodedImage,
    width: u32,
    height: u32,
    policy: ImageEditPolicy,
) -> Result<DecodedImage, ImageEditError> {
    if width == 0 || height == 0 {
        return Err(ImageEditError::InvalidResize);
    }
    checked_output_len(width, height, policy)?;
    let buffer = RgbaImage::from_raw(source.width, source.height, source.pixels_bgra8.clone())
        .ok_or(ImageEditError::InvalidPixelBuffer)?;
    let resized = image::imageops::resize(
        &buffer,
        width,
        height,
        image::imageops::FilterType::Lanczos3,
    );
    Ok(DecodedImage {
        pixels_bgra8: resized.into_raw(),
        width,
        height,
    })
}

fn validate_source(source: &DecodedImage) -> Result<(), ImageEditError> {
    let expected = (source.width as usize)
        .checked_mul(source.height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(ImageEditError::InvalidPixelBuffer)?;
    if source.width == 0 || source.height == 0 || source.pixels_bgra8.len() != expected {
        Err(ImageEditError::InvalidPixelBuffer)
    } else {
        Ok(())
    }
}

fn checked_output_len(
    width: u32,
    height: u32,
    policy: ImageEditPolicy,
) -> Result<usize, ImageEditError> {
    let bytes = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .unwrap_or(u64::MAX);
    if bytes > policy.max_output_bytes {
        return Err(ImageEditError::MemoryLimit {
            bytes,
            limit: policy.max_output_bytes,
        });
    }
    usize::try_from(bytes).map_err(|_| ImageEditError::MemoryLimit {
        bytes,
        limit: policy.max_output_bytes,
    })
}

fn bgra_to_rgba(pixels: &[u8]) -> Vec<u8> {
    let mut rgba = pixels.to_vec();
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    rgba
}

fn flatten_rgba_on_white(rgba: &[u8]) -> Vec<u8> {
    let mut rgb = Vec::with_capacity(rgba.len() / 4 * 3);
    for pixel in rgba.chunks_exact(4) {
        let alpha = u16::from(pixel[3]);
        for channel in &pixel[..3] {
            let blended = (u16::from(*channel) * alpha + 255 * (255 - alpha) + 127) / 255;
            rgb.push(blended as u8);
        }
    }
    rgb
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> DecodedImage {
        DecodedImage {
            pixels_bgra8: vec![1, 0, 0, 255, 2, 0, 0, 255, 3, 0, 0, 255, 4, 0, 0, 255],
            width: 2,
            height: 2,
        }
    }

    #[test]
    fn crop_copies_the_requested_pixel_rectangle() {
        let cropped = apply_image_edit(
            &source(),
            ImageEditOperation::Crop(CropRect::new(1, 0, 1, 2)),
            ImageEditPolicy::default(),
        )
        .unwrap();
        assert_eq!((cropped.width, cropped.height), (1, 2));
        assert_eq!(cropped.pixels_bgra8, [2, 0, 0, 255, 4, 0, 0, 255]);
    }

    #[test]
    fn invalid_edits_and_large_outputs_are_rejected() {
        assert!(matches!(
            apply_image_edit(
                &source(),
                ImageEditOperation::Crop(CropRect::new(1, 1, 2, 2)),
                ImageEditPolicy::default()
            ),
            Err(ImageEditError::InvalidCrop)
        ));
        assert!(matches!(
            apply_image_edit(
                &source(),
                ImageEditOperation::Resize {
                    width: 4,
                    height: 4
                },
                ImageEditPolicy {
                    max_output_bytes: 63
                }
            ),
            Err(ImageEditError::MemoryLimit { .. })
        ));
    }

    #[test]
    fn resize_has_the_requested_dimensions() {
        let resized = apply_image_edit(
            &source(),
            ImageEditOperation::Resize {
                width: 4,
                height: 3,
            },
            ImageEditPolicy::default(),
        )
        .unwrap();
        assert_eq!((resized.width, resized.height), (4, 3));
        assert_eq!(resized.pixels_bgra8.len(), 4 * 3 * 4);
    }

    #[test]
    fn pixel_layout_conversion_and_alpha_flattening_are_explicit() {
        assert_eq!(bgra_to_rgba(&[30, 20, 10, 128]), [10, 20, 30, 128]);
        assert_eq!(
            flatten_rgba_on_white(&[0, 0, 0, 0, 10, 20, 30, 255]),
            [255, 255, 255, 10, 20, 30]
        );
    }

    #[test]
    fn supported_export_formats_write_decodable_images() {
        for format in [
            ImageExportFormat::Png,
            ImageExportFormat::Jpeg,
            ImageExportFormat::WebP,
        ] {
            let path = std::env::temp_dir().join(format!(
                "lumia-edit-export-{}.{}",
                uuid::Uuid::now_v7(),
                format.extension()
            ));
            export_decoded_image(&source(), &path, format).unwrap();
            let exported = image::open(&path).unwrap();
            assert_eq!((exported.width(), exported.height()), (2, 2));
            std::fs::remove_file(path).unwrap();
        }
    }
}
