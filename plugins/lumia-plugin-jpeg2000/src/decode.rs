use std::fs::File;
use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat};
use jpeg2k::{DecodeParameters, Image};
use lumia_plugin_api::{DecodePreviewParams, DecodePreviewResult, ImageOutput, PluginErrorKind};

use crate::probe::{probe, read_header, ProbeError};

const MAX_DECODE_ALLOC: u64 = 256 * 1024 * 1024;
const MAX_REDUCE_LEVEL: u32 = 8;

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

    let header = read_header(&params.input.path)?;
    if header.components > 4 || header.max_precision > 16 {
        return Err(DecodeError::UnsupportedImage);
    }
    let reduce = reduction_level(
        header.width,
        header.height,
        params.max_width,
        params.max_height,
    );
    validate_decode_budget(&header, reduce)?;
    let decode_params = DecodeParameters::new().reduce(reduce).strict(true);
    let decoded = Image::from_file_with(&params.input.path, decode_params).or_else(|error| {
        if reduce > 0 && validate_decode_budget(&header, 0).is_ok() {
            Image::from_file_with(&params.input.path, DecodeParameters::new().strict(true))
        } else {
            Err(error)
        }
    })?;
    let image = DynamicImage::try_from(&decoded)?.to_rgba8();
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
        format_name: probe(&params.input.path)?.format_name,
    })
}

fn validate_decode_budget(
    header: &crate::probe::Jpeg2000Header,
    reduce: u32,
) -> Result<(), DecodeError> {
    let divisor = 1_u64 << reduce;
    let width = u64::from(header.width).div_ceil(divisor);
    let height = u64::from(header.height).div_ceil(divisor);
    let bytes = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(u64::from(header.components)))
        .and_then(|samples| samples.checked_mul(4))
        .ok_or(DecodeError::ResourceLimit)?;
    if bytes > MAX_DECODE_ALLOC {
        Err(DecodeError::ResourceLimit)
    } else {
        Ok(())
    }
}

fn reduction_level(width: u32, height: u32, max_width: u32, max_height: u32) -> u32 {
    let mut reduce = 0_u32;
    while reduce < MAX_REDUCE_LEVEL
        && (u64::from(width).div_ceil(1_u64 << reduce) > u64::from(max_width)
            || u64::from(height).div_ceil(1_u64 << reduce) > u64::from(max_height))
    {
        reduce += 1;
    }
    reduce
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
    #[error("JPEG 2000 image uses unsupported components or precision")]
    UnsupportedImage,
    #[error("JPEG 2000 image exceeds preview resource limits")]
    ResourceLimit,
    #[error(transparent)]
    Probe(#[from] ProbeError),
    #[error("could not read or write JPEG 2000 preview data")]
    Io(#[from] std::io::Error),
    #[error("JPEG 2000 image could not be decoded")]
    Jpeg2000(#[from] jpeg2k::error::Error),
    #[error("JPEG 2000 preview PNG could not be encoded")]
    Image(#[from] image::ImageError),
}

impl DecodeError {
    pub(crate) fn kind(&self) -> PluginErrorKind {
        match self {
            Self::InvalidBounds | Self::ResourceLimit => PluginErrorKind::ResourceLimit,
            Self::UnsupportedImage => PluginErrorKind::UnsupportedFormat,
            Self::Probe(ProbeError::UnsupportedSignature) => PluginErrorKind::UnsupportedFormat,
            Self::Probe(ProbeError::ResourceLimit) => PluginErrorKind::ResourceLimit,
            Self::Probe(_) | Self::Jpeg2000(_) => PluginErrorKind::CorruptImage,
            Self::InvalidOutputPath | Self::Io(_) => PluginErrorKind::PluginUnavailable,
            Self::Image(_) => PluginErrorKind::DecodeFailed,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::GenericImageView;
    use lumia_plugin_api::ImagePath;

    use super::*;

    const JP2_FIXTURE: &[u8] = &[
        0x00, 0x00, 0x00, 0x0c, 0x6a, 0x50, 0x20, 0x20, 0x0d, 0x0a, 0x87, 0x0a, 0x00, 0x00, 0x00,
        0x14, 0x66, 0x74, 0x79, 0x70, 0x6a, 0x70, 0x32, 0x20, 0x00, 0x00, 0x00, 0x00, 0x6a, 0x70,
        0x32, 0x20, 0x00, 0x00, 0x00, 0x2d, 0x6a, 0x70, 0x32, 0x68, 0x00, 0x00, 0x00, 0x16, 0x69,
        0x68, 0x64, 0x72, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x04, 0x00, 0x03, 0x07, 0x07,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x0f, 0x63, 0x6f, 0x6c, 0x72, 0x01, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x10, 0x00, 0x00, 0x00, 0xa3, 0x6a, 0x70, 0x32, 0x63, 0xff, 0x4f, 0xff, 0x51, 0x00,
        0x2f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x07, 0x01, 0x01, 0x07, 0x01, 0x01, 0x07, 0x01,
        0x01, 0xff, 0x52, 0x00, 0x0c, 0x00, 0x00, 0x00, 0x01, 0x01, 0x00, 0x04, 0x04, 0x00, 0x01,
        0xff, 0x5c, 0x00, 0x04, 0x40, 0x40, 0xff, 0x64, 0x00, 0x25, 0x00, 0x01, 0x43, 0x72, 0x65,
        0x61, 0x74, 0x65, 0x64, 0x20, 0x62, 0x79, 0x20, 0x4f, 0x70, 0x65, 0x6e, 0x4a, 0x50, 0x45,
        0x47, 0x20, 0x76, 0x65, 0x72, 0x73, 0x69, 0x6f, 0x6e, 0x20, 0x32, 0x2e, 0x35, 0x2e, 0x32,
        0xff, 0x90, 0x00, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00, 0x2b, 0x00, 0x01, 0xff, 0x93, 0xc7,
        0xd4, 0x0c, 0x08, 0x90, 0xab, 0xde, 0x7c, 0x1f, 0xc7, 0xd4, 0x0c, 0x01, 0xd8, 0x80, 0x04,
        0x04, 0x23, 0xdf, 0x80, 0x40, 0x01, 0xd8, 0x82, 0xb4, 0x43, 0x66, 0x85, 0x3f, 0xff, 0xd9,
    ];

    fn temp_dir() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("lumia-jpeg2000-{nonce}"))
    }

    #[test]
    fn jp2_and_raw_codestream_headers_are_detected() {
        let dir = temp_dir();
        std::fs::create_dir(&dir).unwrap();
        let jp2 = dir.join("input.jp2");
        let j2k = dir.join("input.j2k");
        std::fs::write(&jp2, JP2_FIXTURE).unwrap();
        let codestream = JP2_FIXTURE
            .windows(4)
            .position(|bytes| bytes == [0xff, 0x4f, 0xff, 0x51])
            .unwrap();
        std::fs::write(&j2k, &JP2_FIXTURE[codestream..]).unwrap();

        for path in [&jp2, &j2k] {
            let header = read_header(path).unwrap();
            assert_eq!((header.width, header.height), (4, 2));
            assert_eq!(header.components, 3);
            assert_eq!(header.max_precision, 8);
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn decode_writes_a_bounded_png() {
        let dir = temp_dir();
        std::fs::create_dir(&dir).unwrap();
        let input = dir.join("input.jp2");
        let output = dir.join("preview.png");
        std::fs::write(&input, JP2_FIXTURE).unwrap();

        let result = decode_preview(DecodePreviewParams {
            input: ImagePath {
                path: input,
                media_type: Some("image/jp2".to_string()),
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
}
