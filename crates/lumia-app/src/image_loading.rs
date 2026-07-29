use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{Context, Window};
use lumia_core::{FolderNavigation, ImageDocument};

use crate::app::LumiaApp;
use crate::large_image::{large_image_cache_dir, should_decode_large_image};
use crate::load_state::PreparedImage;
use crate::professional_decode::is_photoshop_path;
use crate::util::{format_large_image_error, format_load_error};

const MAX_ANIMATION_FRAME_BYTES: u64 = 48 * 1024 * 1024;

impl LumiaApp {
    pub(crate) fn load_image(
        &mut self,
        path: PathBuf,
        mut window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        self.close_plugin_session(cx);
        self.close_edit_session(false, cx);
        self.cancel_preview_preloads();
        let cached_preview = self.lookup_cached_preview(&path, cx);
        let generation = self.loads.begin_current_load();
        self.large_image.reset();
        let retired_tiles = self.large_image.drain_retired_tiles().collect::<Vec<_>>();
        for image in retired_tiles {
            cx.drop_image(image.render_image(), window.as_deref_mut());
        }
        self.release_retired_images(window.as_deref_mut(), cx);
        self.viewer
            .replace_document(ImageDocument::from_path(&path));
        self.ui.error_message = None;
        self.ui.show_zoom_menu = false;
        self.ui.is_panning = false;
        self.ui.is_overview_panning = false;
        self.ui.context_menu_position = None;
        self.ui.last_mouse_position = None;
        self.window_title = self.current_window_title();
        if let Some(window) = window.as_deref_mut() {
            window.set_window_title(&self.window_title);
        }
        let Some(cancellation) = self.loads.begin_decode(generation) else {
            return;
        };
        cx.notify();

        let probe_path = path.clone();
        let catalog_path = path.clone();
        let needs_catalog_scan = !self.navigation.contains(&path);
        if let Some(cached) = cached_preview {
            let metadata = cached.metadata().clone();
            self.loads
                .set_source_dimensions(generation, Some((metadata.width, metadata.height)));
            self.loads.set_file_metadata(cached.file().clone());
            if let Some(document) = self.viewer.document_mut() {
                document.metadata = Some(metadata);
            }
            self.large_image
                .begin(path.clone(), generation, cancellation);
            self.loads.set_current_image(generation, cached.image());
            self.large_image.mark_preview_ready(generation);
            self.loads.finish_decode(generation);
            self.ui.error_message = None;
            self.release_retired_images(window.as_deref_mut(), cx);
            if needs_catalog_scan {
                self.start_navigation_scan(path, generation, cx);
            } else {
                self.schedule_adjacent_preloads(cx);
            }
            cx.notify();
            return;
        }

        cx.spawn(async move |this, cx| {
            let probe_task = cx
                .background_executor()
                .spawn(async move { ImageDocument::probe_from_path(&probe_path) });
            let catalog_task = needs_catalog_scan.then(|| {
                cx.background_executor()
                    .spawn(async move { FolderNavigation::scan(&catalog_path) })
            });
            let probe = probe_task.await;

            let should_continue = this
                .update(cx, |this, cx| {
                    if !this.loads.is_current(generation)
                        || this.image_path() != Some(path.as_path())
                    {
                        return false;
                    }
                    let mut probe = match probe {
                        Ok(probe) => probe,
                        Err(error) => {
                            this.loads.finish_decode(generation);
                            this.loads.clear_display_images();
                            this.release_retired_images(None, cx);
                            this.ui.error_message = Some(format_load_error(&error));
                            cx.notify();
                            return false;
                        }
                    };
                    let needs_heif_decode = is_heif(&path);
                    let needs_photoshop_decode = is_photoshop_path(&path);
                    let needs_large_image_decode =
                        probe.document.metadata.as_ref().is_some_and(|metadata| {
                            let decoded_bytes = u64::from(metadata.width)
                                .saturating_mul(u64::from(metadata.height))
                                .saturating_mul(4);
                            should_decode_large_image(&path, metadata.width, metadata.height)
                                || (is_gif(&path) && decoded_bytes > MAX_ANIMATION_FRAME_BYTES)
                        });
                    let allow_full_heif_decode =
                        probe.document.metadata.as_ref().is_none_or(|metadata| {
                            u64::from(metadata.width)
                                .saturating_mul(u64::from(metadata.height))
                                .saturating_mul(4)
                                <= lumia_core::DecodePolicy::default().max_output_bytes
                        });
                    let source_dimensions = probe
                        .document
                        .metadata
                        .as_ref()
                        .map(|metadata| (metadata.width, metadata.height));
                    this.loads
                        .set_source_dimensions(generation, source_dimensions);
                    let heif_bytes = probe.document.heif_bytes.take();
                    this.loads.set_file_metadata(probe.file);
                    if let Some(document) = this.viewer.document_mut() {
                        *document = probe.document;
                    }
                    if needs_large_image_decode {
                        this.large_image
                            .begin(path.clone(), generation, cancellation.clone());
                        this.start_current_large_image_decode(
                            path.clone(),
                            generation,
                            cancellation.clone(),
                            cx,
                        );
                    } else if needs_photoshop_decode {
                        this.start_current_photoshop_decode(
                            path.clone(),
                            generation,
                            cancellation.clone(),
                            cx,
                        );
                    } else if needs_heif_decode {
                        this.start_current_decode(
                            path.clone(),
                            generation,
                            heif_bytes,
                            allow_full_heif_decode,
                            cancellation.clone(),
                            cx,
                        );
                    } else if is_gif(&path) {
                        this.start_current_gif_decode(
                            path.clone(),
                            generation,
                            cancellation.clone(),
                            cx,
                        );
                    } else if is_svg(&path) {
                        this.loads.finish_decode(generation);
                        this.ui.error_message = None;
                    } else {
                        this.start_current_static_decode(
                            path.clone(),
                            generation,
                            cancellation.clone(),
                            cx,
                        );
                    }
                    cx.notify();
                    true
                })
                .unwrap_or(false);

            if let Some(catalog_task) = catalog_task {
                let catalog = catalog_task.await;
                if !should_continue {
                    return;
                }
                let _ = this.update(cx, |this, cx| {
                    if this.loads.is_current(generation)
                        && this.image_path() == Some(path.as_path())
                    {
                        this.navigation = catalog.unwrap_or_default();
                        this.schedule_adjacent_preloads(cx);
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    fn start_navigation_scan(&mut self, path: PathBuf, generation: u64, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            let scan_path = path.clone();
            let catalog = cx
                .background_executor()
                .spawn(async move { FolderNavigation::scan(&scan_path) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.loads.is_current(generation) && this.image_path() == Some(path.as_path()) {
                    this.navigation = catalog.unwrap_or_default();
                    this.schedule_adjacent_preloads(cx);
                    cx.notify();
                }
            });
        })
        .detach();
    }
    pub(crate) fn release_retired_images(
        &mut self,
        mut current_window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        // GPUI temporarily removes a window from App::windows while that
        // window is being updated, so synchronous actions must pass it here
        // explicitly or its sprite-atlas textures will not be removed.
        for image in self.loads.drain_retired_images() {
            cx.drop_image(image.render_image(), current_window.as_deref_mut());
        }
    }

    fn start_current_large_image_decode(
        &mut self,
        path: PathBuf,
        generation: u64,
        cancellation: lumia_core::DecodeCancellation,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let preview_path = path.clone();
            let preview_cache = large_image_cache_dir();
            let preview_cancellation = cancellation.clone();
            let preview = cx
                .background_executor()
                .spawn(async move {
                    lumia_core::decode_large_image_preview(
                        &preview_path,
                        2048,
                        2048,
                        &preview_cache,
                        &preview_cancellation,
                    )
                    .map(PreparedImage::from_decoded)
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                if !this.loads.is_current(generation)
                    || !this.large_image.matches(generation, &path)
                {
                    return;
                }
                match preview {
                    Ok(preview) => {
                        let cached_image = preview.clone();
                        let cached_metadata = this
                            .viewer
                            .document()
                            .and_then(|document| document.metadata.clone());
                        let cached_file = this.loads.file_metadata().cloned();
                        this.loads.set_current_image(generation, preview);
                        this.release_retired_images(None, cx);
                        if let (Some(metadata), Some(file)) = (cached_metadata, cached_file) {
                            this.store_cached_preview(
                                path.clone(),
                                cached_image,
                                metadata,
                                file,
                                cx,
                            );
                        }
                        this.large_image.mark_preview_ready(generation);
                        this.loads.finish_decode(generation);
                        this.ui.error_message = None;
                        this.schedule_adjacent_preloads(cx);
                        cx.notify();
                    }
                    Err(_error) if cancellation.is_cancelled() => {}
                    Err(error) => {
                        this.loads.clear_display_images();
                        this.release_retired_images(None, cx);
                        this.loads.finish_decode(generation);
                        this.ui.error_message =
                            Some(format_large_image_error(this.settings.language, &error));
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    pub(crate) fn start_large_image_raster_build(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.image_path().map(Path::to_path_buf) else {
            self.large_image.finish_raster_build();
            return;
        };
        let generation = self.large_image.generation();
        let Some(cancellation) = self.large_image.cancellation() else {
            self.large_image.finish_raster_build();
            return;
        };
        cx.spawn(async move |this, cx| {
            let raster_path = path.clone();
            let raster_cache = large_image_cache_dir();
            let raster_cancellation = cancellation.clone();
            let raster = cx
                .background_executor()
                .spawn(async move {
                    lumia_core::build_large_image_raster(
                        &raster_path,
                        &raster_cache,
                        &raster_cancellation,
                    )
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if !this.large_image.matches(generation, &path) {
                    return;
                }
                match raster {
                    Ok(raster) => {
                        this.large_image.install_raster(generation, raster);
                        this.start_large_image_tile_jobs(cx);
                    }
                    Err(_error) if cancellation.is_cancelled() => {
                        this.large_image.finish_raster_build();
                    }
                    Err(error) => {
                        this.large_image.finish_raster_build();
                        this.large_image.record_detail_error(
                            generation,
                            format_large_image_error(this.settings.language, &error),
                        );
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn start_current_decode(
        &mut self,
        path: PathBuf,
        generation: u64,
        heif_bytes: Option<Vec<u8>>,
        allow_full_decode: bool,
        cancellation: lumia_core::DecodeCancellation,
        cx: &mut Context<Self>,
    ) {
        let Some(heif_bytes) = heif_bytes else {
            self.loads.finish_decode(generation);
            self.ui.error_message = Some("HEIF image bytes are unavailable".into());
            cx.notify();
            return;
        };
        let heif_bytes = Arc::new(heif_bytes);
        cx.spawn(async move |this, cx| {
            let preview_bytes = heif_bytes.clone();
            let preview = cx
                .background_executor()
                .spawn(async move {
                    lumia_core::decode_heic_thumbnail(&preview_bytes)
                        .ok()
                        .flatten()
                        .map(PreparedImage::from_decoded)
                })
                .await;

            let should_continue = this
                .update(cx, |this, cx| {
                    if !this.loads.is_current(generation)
                        || this.image_path() != Some(path.as_path())
                    {
                        return false;
                    }
                    if let Some(preview) = preview {
                        this.loads.set_current_image(generation, preview);
                        this.release_retired_images(None, cx);
                        if this.viewer.rotation_quarter_turns() != 0 {
                            this.rebuild_rotated_image(None, cx);
                        }
                        cx.notify();
                    }
                    if !allow_full_decode {
                        this.loads.finish_decode(generation);
                    }
                    true
                })
                .unwrap_or(false);
            if !should_continue || !allow_full_decode {
                return;
            }

            let decode_cancellation = cancellation.clone();
            let full_image = cx
                .background_executor()
                .spawn(async move {
                    lumia_core::decode_heic_with_cancellation(&heif_bytes, &decode_cancellation)
                        .map(PreparedImage::from_decoded)
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                if !this.loads.finish_decode(generation)
                    || this.image_path() != Some(path.as_path())
                {
                    return;
                }
                match full_image {
                    Ok(image) => {
                        this.loads.set_current_image(generation, image);
                        this.release_retired_images(None, cx);
                        this.ui.error_message = None;
                        if this.viewer.rotation_quarter_turns() != 0 {
                            this.rebuild_rotated_image(None, cx);
                        }
                    }
                    Err(_error) if cancellation.is_cancelled() => return,
                    Err(error) => {
                        this.ui.error_message = Some(format_load_error(&error));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

fn is_heif(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("heic") || extension.eq_ignore_ascii_case("heif")
        })
}

fn is_gif(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("gif"))
}

fn is_svg(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_paths_bypass_raster_decode_case_insensitively() {
        assert!(is_svg(Path::new("vector.svg")));
        assert!(is_svg(Path::new("vector.SVG")));
        assert!(!is_svg(Path::new("vector.png")));
    }
}
