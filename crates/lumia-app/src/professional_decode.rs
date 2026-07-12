use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use gpui::Context;
use lumia_core::{ColorDescription, ImageMetadata, PixelFormat, TransferFunction};
use lumia_plugin_api::{
    DecodePreviewParams, DecodePreviewResult, ImagePath, ProbeParams, ProbeResult,
};
use lumia_plugin_host::PluginProcess;

use crate::app::LumiaApp;
use crate::i18n::{tr, TextKey};
use crate::load_state::PreparedImage;
use crate::plugin_catalog::photoshop_manifest;

const MAX_PREVIEW_SIDE: u32 = 8192;

pub(crate) fn is_photoshop_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(lumia_core::requires_plugin_preview_extension)
}

impl LumiaApp {
    pub(crate) fn start_current_photoshop_decode(
        &mut self,
        path: PathBuf,
        generation: u64,
        cancellation: lumia_core::DecodeCancellation,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let decode_path = path.clone();
            let result = cx
                .background_executor()
                .spawn(async move { decode_photoshop(&decode_path) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if !this.loads.finish_decode(generation)
                    || this.image_path() != Some(path.as_path())
                    || cancellation.is_cancelled()
                {
                    return;
                }
                match result {
                    Ok(preview) => {
                        if let Some(document) = this.viewer.document_mut() {
                            document.metadata = Some(preview.metadata);
                        }
                        this.loads.set_current_image(generation, preview.image);
                        this.ui.error_message = None;
                        if this.viewer.rotation_quarter_turns() != 0 {
                            this.rebuild_rotated_image();
                        }
                    }
                    Err(error) => {
                        this.ui.error_message = Some(format!(
                            "{}: {error:#}",
                            tr(this.settings.language, TextKey::PhotoshopPreviewFailed)
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

struct ProfessionalPreview {
    image: PreparedImage,
    metadata: ImageMetadata,
}

fn decode_photoshop(path: &Path) -> Result<ProfessionalPreview> {
    let manifest = photoshop_manifest()?;
    let mut process = PluginProcess::spawn(&manifest).context("start Photoshop preview plugin")?;
    process
        .initialize_for(&manifest)
        .context("initialize Photoshop preview plugin")?;

    let input = ImagePath {
        path: path.to_path_buf(),
        media_type: Some("image/vnd.adobe.photoshop".to_string()),
    };
    let probe: ProbeResult = process
        .request(
            "image.probe",
            ProbeParams {
                input: input.clone(),
            },
        )
        .context("probe Photoshop document")?;
    if !probe.can_decode {
        anyhow::bail!("plugin cannot decode this Photoshop document");
    }

    let output_path = preview_cache_path(path)?;
    if !output_path.is_file() {
        let result: DecodePreviewResult = process
            .request(
                "image.decode_preview",
                DecodePreviewParams {
                    input,
                    output_path: output_path.clone(),
                    max_width: MAX_PREVIEW_SIDE,
                    max_height: MAX_PREVIEW_SIDE,
                },
            )
            .context("decode Photoshop composite preview")?;
        if result.output.path != output_path
            || result.output.media_type.as_deref() != Some("image/png")
        {
            anyhow::bail!("plugin returned an unexpected preview output");
        }
    }

    let decoded = lumia_core::load_decoded_image_from_path(&output_path)
        .context("load Photoshop preview PNG")?;
    let metadata = ImageMetadata {
        width: probe.width.unwrap_or(decoded.width),
        height: probe.height.unwrap_or(decoded.height),
        color: ColorDescription {
            pixel_format: PixelFormat::U8,
            transfer: TransferFunction::Srgb,
            has_alpha: true,
        },
        format_name: probe.format_name,
    };
    Ok(ProfessionalPreview {
        image: PreparedImage::from_decoded(decoded),
        metadata,
    })
}

fn preview_cache_path(path: &Path) -> Result<PathBuf> {
    let metadata = std::fs::metadata(path).context("read Photoshop file metadata")?;
    let mut hasher = DefaultHasher::new();
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .hash(&mut hasher);
    metadata.len().hash(&mut hasher);
    metadata.modified().ok().hash(&mut hasher);
    MAX_PREVIEW_SIDE.hash(&mut hasher);

    let directory = std::env::temp_dir()
        .join("lumia")
        .join("photoshop-previews");
    std::fs::create_dir_all(&directory).context("create Photoshop preview cache")?;
    Ok(directory.join(format!("{:016x}.png", hasher.finish())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn photoshop_path_matching_is_case_insensitive() {
        assert!(is_photoshop_path(Path::new("image.PSD")));
        assert!(is_photoshop_path(Path::new("large.psb")));
        assert!(!is_photoshop_path(Path::new("image.png")));
    }
}
