use std::fs::File;
use std::path::{Path, PathBuf};

use ag_psd::psd::ReadOptions;
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat, RgbaImage};
use lumia_plugin_api::{DecodePreviewParams, DecodePreviewResult, ImageOutput, PluginErrorKind};

use crate::header::PhotoshopHeader;
use crate::probe::{probe, ProbeError};

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
    let bytes = std::fs::read(&params.input.path)?;
    let header = PhotoshopHeader::parse(&bytes, bytes.len() as u64).map_err(ProbeError::from)?;
    if header.depth == 32 || !matches!(header.color_mode, 0 | 1 | 2 | 3) {
        return Err(DecodeError::UnsupportedDocument);
    }

    let document = ag_psd::read_psd(
        &bytes,
        &ReadOptions {
            skip_layer_image_data: Some(true),
            skip_thumbnail: Some(true),
            skip_linked_files_data: Some(true),
            use_image_data: Some(true),
            ..Default::default()
        },
    )?;
    let pixels = document
        .image_data
        .or(document.canvas)
        .ok_or(DecodeError::MissingComposite)?;
    if pixels.width != header.width
        || pixels.height != header.height
        || pixels.data.len()
            != usize::try_from(u64::from(pixels.width) * u64::from(pixels.height) * 4)
                .map_err(|_| DecodeError::InvalidPixelData)?
    {
        return Err(DecodeError::InvalidPixelData);
    }

    let image = RgbaImage::from_raw(pixels.width, pixels.height, pixels.data)
        .ok_or(DecodeError::InvalidPixelData)?;
    let (width, height) = bounded_dimensions(
        image.width(),
        image.height(),
        params.max_width,
        params.max_height,
    );
    let image = if (width, height) == image.dimensions() {
        image
    } else {
        image::imageops::resize(&image, width, height, FilterType::Triangle)
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
        width,
        height,
        format_name: probe_result.format_name,
    })
}

pub(crate) fn bounded_dimensions(
    width: u32,
    height: u32,
    max_width: u32,
    max_height: u32,
) -> (u32, u32) {
    if width <= max_width && height <= max_height {
        return (width, height);
    }
    let scale = (max_width as f64 / width as f64).min(max_height as f64 / height as f64);
    (
        ((width as f64 * scale).floor() as u32).max(1),
        ((height as f64 * scale).floor() as u32).max(1),
    )
}

pub(crate) fn partial_path(output: &Path) -> PathBuf {
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
    #[error("Photoshop document uses an unsupported color mode or depth")]
    UnsupportedDocument,
    #[error("Photoshop document does not contain a composite preview")]
    MissingComposite,
    #[error("Photoshop composite pixel data is invalid")]
    InvalidPixelData,
    #[error(transparent)]
    Probe(#[from] ProbeError),
    #[error("could not read or write Photoshop preview data")]
    Io(#[from] std::io::Error),
    #[error("Photoshop document could not be decoded")]
    Psd(#[from] ag_psd::reader::ReadError),
    #[error("Photoshop preview PNG could not be encoded")]
    Image(#[from] image::ImageError),
}

impl DecodeError {
    pub(crate) fn kind(&self) -> PluginErrorKind {
        match self {
            Self::InvalidBounds => PluginErrorKind::ResourceLimit,
            Self::UnsupportedDocument | Self::MissingComposite => {
                PluginErrorKind::UnsupportedFormat
            }
            Self::InvalidPixelData | Self::Psd(_) => PluginErrorKind::CorruptImage,
            Self::Probe(ProbeError::Header(crate::header::HeaderError::ResourceLimit)) => {
                PluginErrorKind::ResourceLimit
            }
            Self::Probe(ProbeError::Header(
                crate::header::HeaderError::InvalidSignature
                | crate::header::HeaderError::UnsupportedVersion(_),
            )) => PluginErrorKind::UnsupportedFormat,
            Self::Probe(ProbeError::Header(_)) => PluginErrorKind::CorruptImage,
            Self::InvalidOutputPath | Self::Probe(ProbeError::Io(_)) | Self::Io(_) => {
                PluginErrorKind::PluginUnavailable
            }
            Self::Image(_) => PluginErrorKind::DecodeFailed,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use ag_psd::psd::{ColorMode, PixelData, Psd, WriteOptions};
    use image::GenericImageView;
    use lumia_plugin_api::{DecodePreviewParams, ImagePath};

    use super::*;

    fn temp_dir() -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("lumia-photoshop-{nonce}"))
    }

    fn fixture(psb: bool) -> Vec<u8> {
        let width = 4;
        let height = 2;
        let mut data = Vec::with_capacity(width * height * 4);
        for _ in 0..width * height {
            data.extend_from_slice(&[220, 40, 80, 255]);
        }
        let psd = Psd {
            width: width as f64,
            height: height as f64,
            channels: Some(4.0),
            bits_per_channel: Some(8.0),
            color_mode: Some(ColorMode::Rgb),
            image_data: Some(PixelData {
                width: width as u32,
                height: height as u32,
                data,
            }),
            ..Default::default()
        };
        ag_psd::write_psd(
            &psd,
            &WriteOptions {
                psb: Some(psb),
                ..Default::default()
            },
        )
    }

    #[test]
    fn bounded_dimensions_preserve_aspect_ratio_without_upscaling() {
        assert_eq!(bounded_dimensions(4000, 2000, 1000, 1000), (1000, 500));
        assert_eq!(bounded_dimensions(400, 200, 1000, 1000), (400, 200));
    }

    #[test]
    fn decode_writes_bounded_png_for_psd_and_psb() {
        let dir = temp_dir();
        std::fs::create_dir(&dir).unwrap();

        for psb in [false, true] {
            let extension = if psb { "psb" } else { "psd" };
            let input = dir.join(format!("input.{extension}"));
            let output = dir.join(format!("output-{extension}.png"));
            std::fs::write(&input, fixture(psb)).unwrap();

            let result = decode_preview(DecodePreviewParams {
                input: ImagePath {
                    path: input,
                    media_type: Some("image/vnd.adobe.photoshop".to_string()),
                },
                output_path: output.clone(),
                max_width: 2,
                max_height: 2,
            })
            .unwrap();

            assert_eq!((result.width, result.height), (2, 1));
            assert_eq!(result.output.path, output);
            assert_eq!(result.output.media_type.as_deref(), Some("image/png"));
            assert_eq!(
                image::open(&result.output.path).unwrap().dimensions(),
                (2, 1)
            );
        }

        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_decode_does_not_leave_partial_output() {
        let dir = temp_dir();
        std::fs::create_dir(&dir).unwrap();
        let input = dir.join("broken.psd");
        let output = dir.join("preview.png");
        std::fs::write(&input, b"not a Photoshop document").unwrap();

        assert!(decode_preview(DecodePreviewParams {
            input: ImagePath {
                path: input,
                media_type: None,
            },
            output_path: output.clone(),
            max_width: 100,
            max_height: 100,
        })
        .is_err());
        assert!(!output.exists());
        assert!(!partial_path(&output).exists());

        std::fs::remove_dir_all(dir).unwrap();
    }
}
