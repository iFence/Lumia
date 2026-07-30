use std::fs::File;
use std::path::{Path, PathBuf};

use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat, RgbImage};
use lumia_plugin_api::{
    DecodePreviewParams, DecodePreviewResult, ImageOutput, PluginErrorKind, PluginGeoCoordinates,
    PluginImageMetadata, ProbeResult,
};

use crate::bridge::{Bridge, BridgeError, BridgeProbe};

pub(crate) const RAW_EXTENSIONS: &[&str] = &[
    "dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "sr2", "srf", "raf", "orf", "rw2", "rwl",
    "pef", "srw", "3fr", "fff", "mef", "mos", "mrw", "kdc", "dcr", "erf", "x3f", "iiq",
];
const MAX_PREVIEW_SIDE: u32 = 4096;
const MAX_DECODED_BYTES: usize = 512 * 1024 * 1024;

pub(crate) fn probe(path: &Path) -> Result<ProbeResult, RawError> {
    validate_input(path)?;
    let result = Bridge::load()?.probe(path)?;
    if result.width == 0 || result.height == 0 {
        return Err(RawError::CorruptImage);
    }
    Ok(ProbeResult {
        can_decode: true,
        format_name: extension(path).map(str::to_string),
        width: Some(result.width),
        height: Some(result.height),
        is_hdr: false,
        metadata: metadata(&result),
    })
}

pub(crate) fn decode_preview(params: DecodePreviewParams) -> Result<DecodePreviewResult, RawError> {
    validate_input(&params.input.path)?;
    validate_output(&params.output_path, params.max_width, params.max_height)?;
    let decoded = Bridge::load()?.decode(&params.input.path)?;
    if decoded.pixels_rgb8.len() > MAX_DECODED_BYTES {
        return Err(RawError::ResourceLimit);
    }
    let image = RgbImage::from_raw(decoded.width, decoded.height, decoded.pixels_rgb8)
        .ok_or(RawError::CorruptImage)?;
    let (width, height) = bounded_dimensions(
        image.width(),
        image.height(),
        params.max_width,
        params.max_height,
    );
    let image = if image.dimensions() == (width, height) {
        image
    } else {
        image::imageops::resize(&image, width, height, FilterType::Triangle)
    };
    write_png_atomic(&image, &params.output_path)?;
    Ok(DecodePreviewResult {
        output: ImageOutput {
            path: params.output_path,
            media_type: Some("image/png".to_string()),
        },
        width,
        height,
        format_name: extension(&params.input.path).map(str::to_string),
    })
}

fn metadata(probe: &BridgeProbe) -> Option<PluginImageMetadata> {
    let value = PluginImageMetadata {
        camera_make: probe.camera_make.clone(),
        camera_model: probe.camera_model.clone(),
        lens: probe.lens.clone(),
        iso: probe.iso,
        exposure_time_seconds: probe.exposure_seconds,
        aperture_f_number: probe.aperture,
        focal_length_mm: probe.focal_length_mm,
        date_taken: probe.date_taken.clone(),
        geo_coordinates: probe
            .location
            .as_ref()
            .map(|location| PluginGeoCoordinates {
                latitude: location.latitude,
                longitude: location.longitude,
                altitude_meters: location.altitude_meters,
            }),
    };
    (value != PluginImageMetadata::default()).then_some(value)
}

fn validate_input(path: &Path) -> Result<(), RawError> {
    if !path.is_file() {
        return Err(RawError::InputUnavailable);
    }
    let Some(extension) = extension(path) else {
        return Err(RawError::UnsupportedFormat);
    };
    if !RAW_EXTENSIONS.contains(&extension) {
        return Err(RawError::UnsupportedFormat);
    }
    Ok(())
}

fn validate_output(path: &Path, max_width: u32, max_height: u32) -> Result<(), RawError> {
    if max_width == 0
        || max_height == 0
        || max_width > MAX_PREVIEW_SIDE
        || max_height > MAX_PREVIEW_SIDE
    {
        return Err(RawError::ResourceLimit);
    }
    path.parent()
        .filter(|parent| parent.is_dir())
        .ok_or(RawError::OutputUnavailable)?;
    Ok(())
}

fn extension(path: &Path) -> Option<&str> {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .and_then(|extension| {
            RAW_EXTENSIONS
                .iter()
                .copied()
                .find(|item| *item == extension)
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

fn write_png_atomic(image: &RgbImage, output: &Path) -> Result<(), RawError> {
    let partial = partial_path(output);
    let result = (|| {
        let mut file = File::create(&partial)?;
        DynamicImage::ImageRgb8(image.clone()).write_to(&mut file, ImageFormat::Png)?;
        file.sync_all()?;
        if output.exists() {
            std::fs::remove_file(output)?;
        }
        std::fs::rename(&partial, output)?;
        Ok::<_, RawError>(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(partial);
    }
    result
}

fn partial_path(output: &Path) -> PathBuf {
    let file_name = output
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("raw-preview.png");
    output.with_file_name(format!("{file_name}.{}.part", uuid::Uuid::now_v7()))
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum RawError {
    #[error("RAW format is unsupported")]
    UnsupportedFormat,
    #[error("RAW input path is unavailable")]
    InputUnavailable,
    #[error("RAW image is corrupt")]
    CorruptImage,
    #[error("RAW image exceeds resource limits")]
    ResourceLimit,
    #[error("RAW preview output path is unavailable")]
    OutputUnavailable,
    #[error(transparent)]
    Bridge(#[from] BridgeError),
    #[error("RAW preview could not be read or written")]
    Io(#[from] std::io::Error),
    #[error("RAW preview PNG could not be encoded")]
    Image(#[from] image::ImageError),
}

impl RawError {
    pub(crate) fn kind(&self) -> PluginErrorKind {
        match self {
            Self::UnsupportedFormat | Self::Bridge(BridgeError::Unsupported(_)) => {
                PluginErrorKind::UnsupportedFormat
            }
            Self::CorruptImage | Self::Bridge(BridgeError::Corrupt(_)) => {
                PluginErrorKind::CorruptImage
            }
            Self::ResourceLimit | Self::Bridge(BridgeError::ResourceLimit(_)) => {
                PluginErrorKind::ResourceLimit
            }
            Self::Bridge(BridgeError::DecodeFailed(_) | BridgeError::InvalidImage)
            | Self::Image(_) => PluginErrorKind::DecodeFailed,
            Self::InputUnavailable
            | Self::OutputUnavailable
            | Self::Bridge(BridgeError::Unavailable(_) | BridgeError::AbiMismatch { .. })
            | Self::Io(_) => PluginErrorKind::PluginUnavailable,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lumia_plugin_api::ImagePath;

    #[test]
    fn extension_matching_is_case_insensitive() {
        assert_eq!(extension(Path::new("photo.CR3")), Some("cr3"));
        assert_eq!(extension(Path::new("photo.jpeg")), None);
    }

    #[test]
    fn dimensions_are_bounded_without_upscaling() {
        assert_eq!(bounded_dimensions(8000, 4000, 4096, 4096), (4096, 2048));
        assert_eq!(bounded_dimensions(100, 50, 4096, 4096), (100, 50));
    }

    #[test]
    fn png_output_is_atomic_and_decodable() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("preview.png");
        let image = RgbImage::from_pixel(4, 2, image::Rgb([10, 20, 30]));
        write_png_atomic(&image, &output).unwrap();
        let decoded = image::open(output).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (4, 2));
        assert!(directory.path().read_dir().unwrap().all(|entry| !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".part")));
    }

    #[test]
    fn cc0_raw_samples_decode_to_bounded_png() {
        let Some(sample_directory) = std::env::var_os("LUMIA_RAW_SAMPLE_DIR").map(PathBuf::from)
        else {
            eprintln!("LUMIA_RAW_SAMPLE_DIR is unset; skipping external RAW samples");
            return;
        };
        let output_directory = tempfile::tempdir().unwrap();

        for extension in ["dng", "cr3", "nef", "arw", "raf"] {
            let input = sample_directory.join(format!("sample.{extension}"));
            let probed = probe(&input).unwrap();
            assert!(probed.width.unwrap_or_default() > 0);
            let metadata = probed.metadata.expect("sample metadata");
            assert!(
                metadata.camera_make.is_some() || metadata.camera_model.is_some(),
                "{extension} sample should expose camera metadata"
            );

            let output = output_directory.path().join(format!("{extension}.png"));
            let result = decode_preview(DecodePreviewParams {
                input: ImagePath {
                    path: input,
                    media_type: None,
                },
                output_path: output.clone(),
                max_width: MAX_PREVIEW_SIDE,
                max_height: MAX_PREVIEW_SIDE,
            })
            .unwrap();
            assert!(result.width <= MAX_PREVIEW_SIDE && result.height <= MAX_PREVIEW_SIDE);
            let decoded = image::open(output).unwrap();
            assert_eq!(
                (decoded.width(), decoded.height()),
                (result.width, result.height)
            );
        }
        let unicode_input = output_directory.path().join("相机样张.DNG");
        std::fs::copy(sample_directory.join("sample.dng"), &unicode_input).unwrap();
        let unicode_probe = probe(&unicode_input).unwrap();
        assert!(unicode_probe.width.unwrap_or_default() > 0);
        assert_eq!(unicode_probe.format_name.as_deref(), Some("dng"));

        let corrupt = sample_directory.join("corrupt.dng");
        let error = decode_preview(DecodePreviewParams {
            input: ImagePath {
                path: corrupt,
                media_type: None,
            },
            output_path: output_directory.path().join("corrupt.png"),
            max_width: MAX_PREVIEW_SIDE,
            max_height: MAX_PREVIEW_SIDE,
        })
        .unwrap_err();
        assert_eq!(error.kind(), PluginErrorKind::CorruptImage);
    }
}
