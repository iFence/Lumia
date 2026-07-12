use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::{Context, Window};
use lumia_core::{FitMode, FolderNavigation, ImageDocument, ViewportState};

use crate::app::LumiaApp;
use crate::large_image::{large_image_cache_dir, should_decode_large_image};
use crate::load_state::PreparedImage;
use crate::professional_decode::is_photoshop_path;
use crate::util::format_load_error;
use crate::{NextImage, PreviousImage};
impl LumiaApp {
    pub(crate) fn load_image(
        &mut self,
        path: PathBuf,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let generation = self.loads.begin_current_load();
        self.large_image.reset();
        match ImageDocument::load_from_path(&path) {
            Ok(mut document) => {
                let needs_heif_decode = is_heif(&path);
                let needs_photoshop_decode = is_photoshop_path(&path);
                let needs_large_image_decode = document.metadata.as_ref().is_some_and(|metadata| {
                    should_decode_large_image(&path, metadata.width, metadata.height)
                });
                let needs_async_decode =
                    needs_heif_decode || needs_photoshop_decode || needs_large_image_decode;
                let heif_bytes = document.heif_bytes.take();
                self.viewer.replace_document(document);
                self.ui.error_message = None;
                self.ui.show_zoom_menu = false;
                self.ui.is_panning = false;
                self.ui.is_overview_panning = false;
                self.ui.context_menu_position = None;
                self.ui.last_mouse_position = None;

                self.navigation = FolderNavigation::scan(&path).unwrap_or_default();
                self.loads.sync_catalog(self.navigation.paths());
                self.window_title = self.image_name();
                if let Some(window) = window {
                    window.set_window_title(&self.window_title);
                }

                if let Some(prepared) = self.loads.take_cached(&path) {
                    self.loads.set_current_image(generation, prepared);
                }

                if needs_async_decode && self.loads.current_image().is_none() {
                    if let Some(cancellation) = self.loads.begin_decode(generation) {
                        if needs_large_image_decode {
                            self.large_image
                                .begin(path.clone(), generation, cancellation.clone());
                            self.start_current_large_image_decode(
                                path.clone(),
                                generation,
                                cancellation,
                                cx,
                            );
                        } else if needs_photoshop_decode {
                            self.start_current_photoshop_decode(
                                path.clone(),
                                generation,
                                cancellation,
                                cx,
                            );
                        } else {
                            self.start_current_decode(
                                path.clone(),
                                generation,
                                heif_bytes,
                                cancellation,
                                cx,
                            );
                        }
                    }
                } else {
                    self.start_preload_adjacent(cx);
                }
            }
            Err(error) => {
                self.ui.error_message = Some(format_load_error(&error));
                self.ui.is_panning = false;
                self.ui.is_overview_panning = false;
                self.ui.context_menu_position = None;
                self.ui.last_mouse_position = None;
                self.ui.show_zoom_menu = false;
            }
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

            let should_build_raster = this
                .update(cx, |this, cx| {
                    if !this.loads.is_current(generation)
                        || !this.large_image.matches(generation, &path)
                    {
                        return false;
                    }
                    match preview {
                        Ok(preview) => {
                            this.loads.set_current_image(generation, preview);
                            this.large_image.mark_preview_ready(generation);
                            this.ui.error_message = None;
                            cx.notify();
                            true
                        }
                        Err(_error) if cancellation.is_cancelled() => false,
                        Err(error) => {
                            this.loads.finish_decode(generation);
                            this.ui.error_message =
                                Some(format!("Could not decode large image: {error}"));
                            cx.notify();
                            false
                        }
                    }
                })
                .unwrap_or(false);
            if !should_build_raster {
                return;
            }

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
                if !this.loads.finish_decode(generation)
                    || !this.large_image.matches(generation, &path)
                {
                    return;
                }
                match raster {
                    Ok(raster) => {
                        this.large_image.install_raster(generation, raster);
                        this.start_large_image_tile_jobs(cx);
                    }
                    Err(_error) if cancellation.is_cancelled() => return,
                    Err(error) => {
                        this.large_image
                            .record_detail_error(generation, error.to_string());
                    }
                }
                this.start_preload_adjacent(cx);
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
                        if this.viewer.rotation_quarter_turns() != 0 {
                            this.rebuild_rotated_image();
                        }
                        cx.notify();
                    }
                    true
                })
                .unwrap_or(false);
            if !should_continue {
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
                        this.ui.error_message = None;
                        if this.viewer.rotation_quarter_turns() != 0 {
                            this.rebuild_rotated_image();
                        }
                        this.start_preload_adjacent(cx);
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

    fn start_preload_adjacent(&mut self, cx: &mut Context<Self>) {
        let Some(current_path) = self.image_path().map(Path::to_path_buf) else {
            return;
        };
        let adjacent = self.navigation.adjacent_paths(&current_path);
        self.loads
            .retain_cache(std::iter::once(current_path).chain(adjacent.iter().cloned()));
        self.loads.prepare_preloads(
            adjacent
                .into_iter()
                .filter(|path| is_heif(path))
                .collect::<Vec<_>>(),
        );
        self.start_next_preload(cx);
    }

    fn start_next_preload(&mut self, cx: &mut Context<Self>) {
        let Some(job) = self.loads.begin_next_preload() else {
            return;
        };
        let decode_path = job.path.clone();
        cx.spawn(async move |this, cx| {
            let cancellation = job.cancellation.clone();
            let prepared = cx
                .background_executor()
                .spawn(async move {
                    std::fs::read(decode_path)
                        .ok()
                        .and_then(|bytes| {
                            lumia_core::decode_heic_with_cancellation(&bytes, &cancellation).ok()
                        })
                        .map(PreparedImage::from_decoded)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this
                    .loads
                    .complete_preload(job.path, job.generation, prepared)
                {
                    this.start_next_preload(cx);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(crate) fn current_image_index(&self) -> Option<usize> {
        self.navigation.current_index(self.image_path()?)
    }

    pub(crate) fn sibling_count(&self) -> usize {
        self.navigation.len()
    }

    pub(crate) fn navigate_image(
        &mut self,
        step: i32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self
            .image_path()
            .and_then(|current| self.navigation.step_path(current, step))
            .map(Path::to_path_buf)
        else {
            return;
        };
        if self.image_path() != Some(path.as_path()) {
            self.load_image(path, Some(window), cx);
        }
    }

    pub(crate) fn next_image(
        &mut self,
        _: &NextImage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_viewer_blocked() {
            return;
        }
        self.navigate_image(1, window, cx);
        cx.notify();
    }

    pub(crate) fn previous_image(
        &mut self,
        _: &PreviousImage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_viewer_blocked() {
            return;
        }
        self.navigate_image(-1, window, cx);
        cx.notify();
    }

    pub(crate) fn load_first_supported_drop(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let path = self.ui.pending_drop_paths.iter().find(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(lumia_core::is_supported_image_extension)
        });
        match path.cloned() {
            Some(path) => self.load_image(path, Some(window), cx),
            None => {
                self.ui.error_message =
                    Some("No supported image found in dropped files".to_string())
            }
        }
        self.ui.pending_drop_paths.clear();
    }

    pub(crate) fn image_path(&self) -> Option<&Path> {
        self.viewer.image_path()
    }

    pub(crate) fn image_name(&self) -> String {
        self.image_path()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .unwrap_or("No image")
            .to_string()
    }

    pub(crate) fn scaled_image_size(&self, window: &Window) -> Option<(f32, f32)> {
        let (image_width, image_height) = self.viewer.display_dimensions()?;
        let scale = self.image_display_scale(window)?;
        Some((image_width as f32 * scale, image_height as f32 * scale))
    }

    pub(crate) fn image_display_scale(&self, window: &Window) -> Option<f32> {
        let (image_width, image_height) = self.viewer.display_dimensions()?;
        let viewport_size = window.viewport_size();
        let available_width = f32::from(viewport_size.width).max(1.0);
        let available_height = f32::from(viewport_size.height).max(1.0);
        Some(display_scale(
            image_width,
            image_height,
            available_width,
            available_height,
            self.viewer.viewport(),
        ))
    }
}

fn display_scale(
    image_width: u32,
    image_height: u32,
    available_width: f32,
    available_height: f32,
    viewport: &ViewportState,
) -> f32 {
    let image_width = image_width as f32;
    let image_height = image_height as f32;
    match viewport.fit_mode {
        FitMode::ActualSize => viewport.zoom,
        FitMode::FitToWindow => (available_width / image_width)
            .min(available_height / image_height)
            .min(1.0),
        FitMode::FitWidth => (available_width / image_width).min(1.0),
    }
}

fn is_heif(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("heic") || extension.eq_ignore_ascii_case("heif")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_scale_distinguishes_fit_and_actual_size() {
        let mut viewport = ViewportState::default();
        assert_eq!(display_scale(2000, 1000, 1000.0, 800.0, &viewport), 0.5);

        viewport.reset_actual_size();
        assert_eq!(display_scale(2000, 1000, 1000.0, 800.0, &viewport), 1.0);
    }
}
