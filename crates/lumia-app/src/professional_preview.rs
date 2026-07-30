use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use lumia_core::{
    ColorDescription, DecodeCancellation, ExifMetadata, ImageMetadata, PixelFormat,
    TransferFunction,
};
use lumia_plugin_api::{
    DecodePreviewParams, DecodePreviewResult, ImagePath, PluginImageMetadata, PluginManifest,
    ProbeParams, ProbeResult,
};
use lumia_plugin_host::{PluginHostError, PluginProcess};

use crate::load_state::PreparedImage;

pub(crate) const MAX_PREVIEW_SIDE: u32 = 4096;
const PREVIEW_CACHE_BYTES: u64 = 512 * 1024 * 1024;
const PROBE_TIMEOUT: Duration = Duration::from_secs(15);
const DECODE_TIMEOUT: Duration = Duration::from_secs(120);
const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";

pub(crate) struct ProfessionalPreview {
    pub(crate) image: PreparedImage,
    pub(crate) metadata: ImageMetadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum ProfessionalDecodeError {
    #[error("unsupported format")]
    UnsupportedFormat,
    #[error("corrupt image")]
    CorruptImage,
    #[error("resource limit exceeded")]
    ResourceLimit,
    #[error("decode failed")]
    DecodeFailed,
    #[error("cancelled")]
    Cancelled,
    #[error("plugin unavailable")]
    PluginUnavailable,
}

pub(crate) fn decode_professional_preview(
    path: &Path,
    manifest: &PluginManifest,
    cancellation: &DecodeCancellation,
) -> Result<ProfessionalPreview, ProfessionalDecodeError> {
    if cancellation.is_cancelled() {
        return Err(ProfessionalDecodeError::Cancelled);
    }
    let mut process = PluginProcess::spawn(manifest).map_err(map_host_error)?;
    process.initialize_for(manifest).map_err(map_host_error)?;

    let input = ImagePath {
        path: path.to_path_buf(),
        media_type: Some(media_type(path).to_string()),
    };
    let probe: ProbeResult = process
        .request_interruptible(
            "image.probe",
            ProbeParams {
                input: input.clone(),
            },
            PROBE_TIMEOUT,
            || cancellation.is_cancelled(),
        )
        .map_err(map_host_error)?;
    if !probe.can_decode {
        return Err(ProfessionalDecodeError::UnsupportedFormat);
    }

    let output_path = preview_cache_path(path, manifest)?;
    let decoded = match load_valid_preview(&output_path, None) {
        Ok(decoded) => decoded,
        Err(_) => {
            if output_path.exists() {
                fs::remove_file(&output_path).map_err(|_| ProfessionalDecodeError::DecodeFailed)?;
            }
            let result: DecodePreviewResult = process
                .request_interruptible(
                    "image.decode_preview",
                    DecodePreviewParams {
                        input,
                        output_path: output_path.clone(),
                        max_width: MAX_PREVIEW_SIDE,
                        max_height: MAX_PREVIEW_SIDE,
                    },
                    DECODE_TIMEOUT,
                    || cancellation.is_cancelled(),
                )
                .map_err(map_host_error)?;
            validate_decode_result(&result, &output_path)?;
            let decoded = load_valid_preview(&output_path, Some((result.width, result.height)))?;
            let directory = output_path
                .parent()
                .ok_or(ProfessionalDecodeError::DecodeFailed)?;
            prune_cache_to(directory, PREVIEW_CACHE_BYTES, Some(&output_path))
                .map_err(|_| ProfessionalDecodeError::DecodeFailed)?;
            decoded
        }
    };

    let metadata = ImageMetadata {
        width: probe
            .width
            .filter(|width| *width > 0)
            .unwrap_or(decoded.width),
        height: probe
            .height
            .filter(|height| *height > 0)
            .unwrap_or(decoded.height),
        color: ColorDescription {
            pixel_format: PixelFormat::U8,
            transfer: TransferFunction::Srgb,
            has_alpha: true,
        },
        format_name: probe.format_name,
        exif: plugin_exif(probe.metadata),
    };
    Ok(ProfessionalPreview {
        image: PreparedImage::from_decoded(decoded),
        metadata,
    })
}

fn map_host_error(error: PluginHostError) -> ProfessionalDecodeError {
    match error {
        PluginHostError::Cancelled
        | PluginHostError::Rpc {
            kind: Some(lumia_plugin_api::PluginErrorKind::Cancelled),
            ..
        } => ProfessionalDecodeError::Cancelled,
        PluginHostError::Rpc {
            kind: Some(lumia_plugin_api::PluginErrorKind::UnsupportedFormat),
            ..
        } => ProfessionalDecodeError::UnsupportedFormat,
        PluginHostError::Rpc {
            kind: Some(lumia_plugin_api::PluginErrorKind::CorruptImage),
            ..
        } => ProfessionalDecodeError::CorruptImage,
        PluginHostError::Rpc {
            kind: Some(lumia_plugin_api::PluginErrorKind::ResourceLimit),
            ..
        } => ProfessionalDecodeError::ResourceLimit,
        PluginHostError::Spawn(_)
        | PluginHostError::Rpc {
            kind: Some(lumia_plugin_api::PluginErrorKind::PluginUnavailable),
            ..
        } => ProfessionalDecodeError::PluginUnavailable,
        _ => ProfessionalDecodeError::DecodeFailed,
    }
}

fn media_type(path: &Path) -> &'static str {
    if path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(lumia_core::is_raw_image_extension)
    {
        "image/x-camera-raw"
    } else {
        "image/vnd.adobe.photoshop"
    }
}

fn validate_decode_result(
    result: &DecodePreviewResult,
    expected_path: &Path,
) -> Result<(), ProfessionalDecodeError> {
    if result.output.path != expected_path
        || result.output.media_type.as_deref() != Some("image/png")
        || result.width == 0
        || result.height == 0
        || result.width > MAX_PREVIEW_SIDE
        || result.height > MAX_PREVIEW_SIDE
    {
        return Err(ProfessionalDecodeError::DecodeFailed);
    }
    let metadata =
        fs::symlink_metadata(expected_path).map_err(|_| ProfessionalDecodeError::DecodeFailed)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(ProfessionalDecodeError::DecodeFailed);
    }
    Ok(())
}

fn load_valid_preview(
    path: &Path,
    expected_dimensions: Option<(u32, u32)>,
) -> Result<lumia_core::DecodedImage, ProfessionalDecodeError> {
    let mut signature = [0_u8; 8];
    fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut signature))
        .map_err(|_| ProfessionalDecodeError::CorruptImage)?;
    if &signature != PNG_SIGNATURE {
        return Err(ProfessionalDecodeError::CorruptImage);
    }
    let decoded = lumia_core::load_decoded_image_from_path(path)
        .map_err(|_| ProfessionalDecodeError::CorruptImage)?;
    if decoded.width == 0
        || decoded.height == 0
        || decoded.width > MAX_PREVIEW_SIDE
        || decoded.height > MAX_PREVIEW_SIDE
        || expected_dimensions
            .is_some_and(|dimensions| dimensions != (decoded.width, decoded.height))
    {
        return Err(ProfessionalDecodeError::DecodeFailed);
    }
    Ok(decoded)
}

fn preview_cache_path(
    path: &Path,
    manifest: &PluginManifest,
) -> Result<PathBuf, ProfessionalDecodeError> {
    let directory = std::env::temp_dir()
        .join("lumia")
        .join("professional-previews");
    fs::create_dir_all(&directory).map_err(|_| ProfessionalDecodeError::DecodeFailed)?;
    prune_cache_to(&directory, PREVIEW_CACHE_BYTES, None)
        .map_err(|_| ProfessionalDecodeError::DecodeFailed)?;
    Ok(directory.join(format!("{:016x}.png", preview_cache_key(path, manifest)?)))
}

fn preview_cache_key(
    path: &Path,
    manifest: &PluginManifest,
) -> Result<u64, ProfessionalDecodeError> {
    let metadata = fs::metadata(path).map_err(|_| ProfessionalDecodeError::DecodeFailed)?;
    let mut hasher = DefaultHasher::new();
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .hash(&mut hasher);
    metadata.len().hash(&mut hasher);
    metadata.modified().ok().hash(&mut hasher);
    manifest.id.hash(&mut hasher);
    manifest.version.hash(&mut hasher);
    MAX_PREVIEW_SIDE.hash(&mut hasher);
    Ok(hasher.finish())
}

fn prune_cache_to(
    directory: &Path,
    maximum_bytes: u64,
    protected: Option<&Path>,
) -> std::io::Result<()> {
    let mut total = 0_u64;
    let mut entries = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if !metadata.is_file() {
            continue;
        }
        total = total.saturating_add(metadata.len());
        entries.push((
            metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            metadata.len(),
            entry.path(),
        ));
    }
    entries.sort_by_key(|(modified, _, _)| *modified);
    for (_, bytes, path) in entries {
        if total <= maximum_bytes {
            break;
        }
        if protected.is_some_and(|protected| protected == path) {
            continue;
        }
        if fs::remove_file(path).is_ok() {
            total = total.saturating_sub(bytes);
        }
    }
    Ok(())
}

fn plugin_exif(metadata: Option<PluginImageMetadata>) -> ExifMetadata {
    let Some(metadata) = metadata else {
        return ExifMetadata {
            color_space: Some("sRGB".into()),
            ..ExifMetadata::default()
        };
    };
    ExifMetadata {
        color_space: Some("sRGB".into()),
        camera_make: metadata.camera_make,
        camera_model: metadata.camera_model,
        lens: metadata.lens,
        date_taken: metadata.date_taken,
        focal_length: metadata
            .focal_length_mm
            .map(|value| format!("{value:.2}mm")),
        exposure_time: metadata.exposure_time_seconds.map(format_exposure_time),
        aperture: metadata
            .aperture_f_number
            .map(|value| format!("f/{value:.1}")),
        iso: metadata.iso.map(|value| value.to_string()),
        gps: metadata.geo_coordinates.map(|coordinates| {
            let mut value = format!("{:.6}, {:.6}", coordinates.latitude, coordinates.longitude);
            if let Some(altitude) = coordinates.altitude_meters {
                value.push_str(&format!(", {altitude:.1}m"));
            }
            value
        }),
        ..ExifMetadata::default()
    }
}

fn format_exposure_time(seconds: f64) -> String {
    if seconds > 0.0 && seconds < 1.0 {
        format!("{seconds:.3}s (1/{:.0})", (1.0 / seconds).round())
    } else {
        format!("{seconds:.1} s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lumia_plugin_api::{PluginCapability, PluginPermission};

    fn manifest(version: &str) -> PluginManifest {
        PluginManifest {
            id: "lumia.raw".into(),
            name: "RAW".into(),
            version: version.into(),
            entry: "raw".into(),
            capabilities: vec![PluginCapability::Probe, PluginCapability::DecodePreview],
            permissions: vec![
                PluginPermission::ReadInputPath,
                PluginPermission::WriteTemporaryOutput,
            ],
            supported_inputs: vec!["image/x-camera-raw".into()],
            supported_extensions: vec!["dng".into()],
            supported_outputs: vec!["image/png".into()],
            contributions: Default::default(),
            assets: Vec::new(),
        }
    }

    #[test]
    fn cache_key_includes_plugin_version() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("sample.dng");
        fs::write(&source, b"raw").unwrap();
        assert_ne!(
            preview_cache_key(&source, &manifest("1.0.0")).unwrap(),
            preview_cache_key(&source, &manifest("1.0.1")).unwrap()
        );
    }

    #[test]
    fn output_contract_rejects_wrong_path_format_and_dimensions() {
        let output = PathBuf::from("expected.png");
        let result = DecodePreviewResult {
            output: lumia_plugin_api::ImageOutput {
                path: PathBuf::from("other.png"),
                media_type: Some("image/jpeg".into()),
            },
            width: 0,
            height: MAX_PREVIEW_SIDE + 1,
            format_name: Some("DNG".into()),
        };
        assert_eq!(
            validate_decode_result(&result, &output),
            Err(ProfessionalDecodeError::DecodeFailed)
        );
    }
}
