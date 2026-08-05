use std::fs::File;
use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use image::{DynamicImage, ImageDecoder, ImageFormat};
use jxl_oxide::integration::JxlDecoder;
use lumia_plugin_api::{DecodePreviewParams, DecodePreviewResult, ImageOutput, PluginErrorKind};

use crate::probe::{probe, ProbeError};

const MAX_DECODE_ALLOC: u64 = 256 * 1024 * 1024;

pub(crate) fn decode_preview(
    params: DecodePreviewParams,
) -> Result<DecodePreviewResult, DecodeError> {
    if params.max_width == 0 || params.max_height == 0 {
        return Err(DecodeError::InvalidBounds);
    }
    params
        .output_path
        .parent()
        .filter(|parent| parent.is_dir())
        .ok_or(DecodeError::InvalidOutputPath)?;

    let probe_result = probe(&params.input.path)?;
    if probe_result.is_hdr {
        return Err(DecodeError::HdrUnsupported);
    }
    let width = probe_result.width.ok_or(DecodeError::InvalidDimensions)?;
    let height = probe_result.height.ok_or(DecodeError::InvalidDimensions)?;
    let decoded_bytes = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(16))
        .ok_or(DecodeError::ResourceLimit)?;
    if decoded_bytes > MAX_DECODE_ALLOC {
        return Err(DecodeError::ResourceLimit);
    }

    let file = File::open(&params.input.path)?;
    let mut decoder = JxlDecoder::new(file)?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    decoder.set_limits(limits)?;
    let image = DynamicImage::from_decoder(decoder)?.to_rgba8();
    let (output_width, output_height) = bounded_dimensions(
        image.width(),
        image.height(),
        params.max_width,
        params.max_height,
    );
    let image = if (output_width, output_height) == image.dimensions() {
        image
    } else {
        image::imageops::resize(&image, output_width, output_height, FilterType::Triangle)
    };

    let partial = partial_path(&params.output_path);
    let write_result = (|| -> Result<(), DecodeError> {
        let mut file = File::create(&partial)?;
        DynamicImage::ImageRgba8(image).write_to(&mut file, ImageFormat::Png)?;
        drop(file);
        std::fs::rename(&partial, &params.output_path)?;
        Ok(())
    })();
    if write_result.is_err() {
        let _ = std::fs::remove_file(&partial);
    }
    write_result?;

    Ok(DecodePreviewResult {
        output: ImageOutput {
            path: params.output_path,
            media_type: Some("image/png".to_string()),
        },
        width: output_width,
        height: output_height,
        format_name: probe_result.format_name,
    })
}

fn bounded_dimensions(width: u32, height: u32, max_width: u32, max_height: u32) -> (u32, u32) {
    if width <= max_width && height <= max_height {
        return (width, height);
    }
    let scale = (max_width as f64 / width as f64).min(max_height as f64 / height as f64);
    (
        ((width as f64 * scale).floor() as u32).max(1),
        ((height as f64 * scale).floor() as u32).max(1),
    )
}

fn partial_path(output: &Path) -> PathBuf {
    let file_name = output
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("preview.png");
    output.with_file_name(format!("{file_name}.part"))
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum DecodeError {
    #[error("preview bounds must be greater than zero")]
    InvalidBounds,
    #[error("preview output directory is unavailable")]
    InvalidOutputPath,
    #[error("JPEG XL image dimensions are invalid")]
    InvalidDimensions,
    #[error("HDR JPEG XL preview is not supported yet")]
    HdrUnsupported,
    #[error("JPEG XL image exceeds preview resource limits")]
    ResourceLimit,
    #[error(transparent)]
    Probe(#[from] ProbeError),
    #[error("could not read or write JPEG XL preview data")]
    Io(#[from] std::io::Error),
    #[error("JPEG XL image or preview could not be decoded or encoded")]
    Image(#[from] image::ImageError),
}

impl DecodeError {
    pub(crate) fn kind(&self) -> PluginErrorKind {
        match self {
            Self::InvalidBounds | Self::ResourceLimit => PluginErrorKind::ResourceLimit,
            Self::HdrUnsupported => PluginErrorKind::UnsupportedFormat,
            Self::InvalidDimensions | Self::Probe(_) => PluginErrorKind::CorruptImage,
            Self::InvalidOutputPath | Self::Io(_) => PluginErrorKind::PluginUnavailable,
            Self::Image(_) => PluginErrorKind::DecodeFailed,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::GenericImageView;
    use lumia_plugin_api::ImagePath;

    use super::*;

    const JXL_FIXTURE: &[u8] = &[
        0xff, 0x0a, 0x08, 0x70, 0x10, 0x09, 0x08, 0x02, 0x01, 0x00, 0xa4, 0x02, 0x4b, 0x18, 0x9b,
        0x9c, 0x71, 0x84, 0x03, 0x38, 0x80, 0x03, 0x38, 0x20, 0x4a, 0xc0, 0x39, 0x05, 0x01, 0x00,
        0x20, 0x44, 0x80, 0x08, 0x10, 0x01, 0x22, 0x40, 0xe4, 0xff, 0x91, 0x7b, 0xfa, 0x1e, 0x5a,
        0x67, 0x57, 0x55, 0x55, 0x55, 0x25, 0x49, 0x92, 0x10, 0x50, 0x77, 0x77, 0x77, 0x77, 0x77,
        0xff, 0xff, 0xff, 0xbf, 0x55, 0x6f, 0x66, 0x66, 0x66, 0x06, 0xfe, 0xdf, 0xbf, 0xe7, 0xbf,
        0x87, 0xc6, 0x9c, 0x73, 0xae, 0xb5, 0xcf, 0xbd, 0x49, 0x92, 0x24, 0x04, 0x54, 0x55, 0x55,
        0x55, 0x55, 0x55, 0xff, 0xff, 0xff, 0xcf, 0xbd, 0xaf, 0xbb, 0xbb, 0xbb, 0x1b, 0xfe, 0xdf,
        0xbf, 0xe7, 0xbf, 0x87, 0xc6, 0x9c, 0x73, 0xae, 0xb5, 0xcf, 0xbd, 0x49, 0x92, 0x24, 0x04,
        0x54, 0x55, 0x55, 0x55, 0x55, 0x55, 0xff, 0xff, 0xff, 0xcf, 0xbd, 0xaf, 0xbb, 0xbb, 0xbb,
        0x1b, 0xfe, 0xdf, 0xbf, 0xe7, 0xbf, 0x87, 0xc6, 0x9c, 0x73, 0xae, 0xb5, 0xcf, 0xbd, 0x49,
        0x92, 0x24, 0x04, 0x54, 0x55, 0x55, 0x55, 0x55, 0x55, 0xff, 0xff, 0xff, 0xcf, 0xbd, 0xaf,
        0xbb, 0xbb, 0xbb, 0xfb, 0x02, 0x21, 0x00, 0x78, 0xd0, 0x7a, 0x9c, 0xb9, 0x41, 0x06, 0x00,
        0x00,
    ];

    static NEXT_TEMP_DIR: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let sequence = NEXT_TEMP_DIR.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "lumia-jpeg-xl-{}-{nonce}-{sequence}",
            std::process::id()
        ))
    }

    #[test]
    fn probe_and_decode_write_a_bounded_png() {
        let dir = temp_dir();
        std::fs::create_dir(&dir).unwrap();
        let input = dir.join("input.jxl");
        let output = dir.join("preview.png");
        std::fs::write(&input, JXL_FIXTURE).unwrap();

        let probe = probe(&input).unwrap();
        assert_eq!((probe.width, probe.height), (Some(4), Some(2)));
        assert!(!probe.is_hdr);
        let result = decode_preview(DecodePreviewParams {
            input: ImagePath {
                path: input,
                media_type: Some("image/jxl".to_string()),
            },
            output_path: output.clone(),
            max_width: 2,
            max_height: 2,
        })
        .unwrap();

        assert_eq!((result.width, result.height), (2, 1));
        assert_eq!(image::open(output).unwrap().dimensions(), (2, 1));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn invalid_bounds_do_not_create_output() {
        let dir = temp_dir();
        std::fs::create_dir(&dir).unwrap();
        let input = dir.join("input.jxl");
        let output = dir.join("preview.png");
        std::fs::write(&input, JXL_FIXTURE).unwrap();

        let result = decode_preview(DecodePreviewParams {
            input: ImagePath {
                path: input,
                media_type: None,
            },
            output_path: output.clone(),
            max_width: 0,
            max_height: 2,
        });
        assert!(matches!(result, Err(DecodeError::InvalidBounds)));
        assert!(!output.exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
