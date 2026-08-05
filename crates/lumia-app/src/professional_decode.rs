use std::path::{Path, PathBuf};

use gpui::Context;
use lumia_core::Language;
use lumia_plugin_api::PluginManifest;

use crate::app::LumiaApp;
use crate::i18n::{tr, TextKey};
use crate::plugin_catalog::{jpeg2000_manifest, jpeg_xl_manifest, photoshop_manifest};
use crate::professional_preview::{decode_professional_preview, ProfessionalDecodeError};

pub(crate) fn is_professional_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(lumia_core::requires_plugin_preview_extension)
}

impl LumiaApp {
    pub(crate) fn start_current_professional_decode(
        &mut self,
        path: PathBuf,
        generation: u64,
        cancellation: lumia_core::DecodeCancellation,
        cx: &mut Context<Self>,
    ) {
        let Some(manifest) = self.professional_decoder_manifest(&path) else {
            self.loads.finish_decode(generation);
            self.ui.error_message = Some(professional_error_message(
                self.settings.language,
                &path,
                ProfessionalDecodeError::PluginUnavailable,
            ));
            cx.notify();
            return;
        };

        cx.spawn(async move |this, cx| {
            let decode_path = path.clone();
            let decode_cancellation = cancellation.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    decode_professional_preview(&decode_path, &manifest, &decode_cancellation)
                })
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
                        this.release_retired_images(None, cx);
                        this.ui.error_message = None;
                        if this.viewer.rotation_quarter_turns() != 0 {
                            this.rebuild_rotated_image(None, cx);
                        }
                    }
                    Err(ProfessionalDecodeError::Cancelled) => {}
                    Err(error) => {
                        this.loads.clear_display_images();
                        this.release_retired_images(None, cx);
                        this.ui.error_message = Some(professional_error_message(
                            this.settings.language,
                            &path,
                            error,
                        ));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn professional_decoder_manifest(&self, path: &Path) -> Option<PluginManifest> {
        let extension = path.extension()?.to_str()?;
        if let Some(plugin) = self.plugins.registry.decoder_for_extension(extension) {
            let mut manifest = plugin.manifest.clone();
            manifest.entry = plugin.entry_path();
            return Some(manifest);
        }
        if extension.eq_ignore_ascii_case("jxl") {
            jpeg_xl_manifest().ok()
        } else if ["jp2", "j2k", "j2c", "jpc"]
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(extension))
        {
            jpeg2000_manifest().ok()
        } else if extension.eq_ignore_ascii_case("psd") || extension.eq_ignore_ascii_case("psb") {
            photoshop_manifest().ok()
        } else {
            None
        }
    }
}

fn professional_error_message(
    language: Language,
    path: &Path,
    error: ProfessionalDecodeError,
) -> String {
    let key = match error {
        ProfessionalDecodeError::UnsupportedFormat => TextKey::ProfessionalUnsupported,
        ProfessionalDecodeError::CorruptImage => TextKey::ProfessionalCorrupt,
        ProfessionalDecodeError::ResourceLimit => TextKey::ProfessionalResourceLimit,
        ProfessionalDecodeError::DecodeFailed | ProfessionalDecodeError::Cancelled => {
            TextKey::ProfessionalDecodeFailed
        }
        ProfessionalDecodeError::PluginUnavailable
            if path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(lumia_core::is_raw_image_extension) =>
        {
            TextKey::ProfessionalPluginUnavailable
        }
        ProfessionalDecodeError::PluginUnavailable => TextKey::ProfessionalBundledPluginUnavailable,
    };
    tr(language, key).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn professional_path_matching_is_case_insensitive() {
        assert!(is_professional_path(Path::new("image.PSD")));
        assert!(is_professional_path(Path::new("large.psb")));
        assert!(is_professional_path(Path::new("photo.DNG")));
        assert!(is_professional_path(Path::new("photo.nEf")));
        assert!(is_professional_path(Path::new("photo.JXL")));
        assert!(is_professional_path(Path::new("scan.JP2")));
        assert!(is_professional_path(Path::new("scan.j2c")));
        assert!(!is_professional_path(Path::new("image.png")));
    }
}
