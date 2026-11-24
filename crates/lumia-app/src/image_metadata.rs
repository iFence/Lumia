use std::path::Path;

use gpui_kit::Context;
use lumia_core::{ExifMetadata, ImageDocument};

use crate::app::LumiaApp;

impl LumiaApp {
    pub(crate) fn install_exif_metadata(
        &mut self,
        generation: u64,
        path: &Path,
        exif: ExifMetadata,
        cx: &mut Context<Self>,
    ) {
        if !self.loads.is_current(generation) || self.image_path() != Some(path) {
            return;
        }
        if let Some(metadata) = self
            .viewer
            .document_mut()
            .and_then(|document| document.metadata.as_mut())
        {
            metadata.exif = exif;
            cx.notify();
        }
    }

    pub(crate) fn start_exif_metadata_load(
        &mut self,
        path: std::path::PathBuf,
        generation: u64,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let metadata_path = path.clone();
            let exif = cx
                .background_executor()
                .spawn(async move { ImageDocument::read_exif_metadata(&metadata_path) })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.install_exif_metadata(generation, &path, exif, cx);
            });
        })
        .detach();
    }
}
