use std::path::{Path, PathBuf};

use gpui::{Context, Window};
use lumia_core::{FolderNavigation, ImageDocument};

use crate::app::LumiaApp;
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
        match ImageDocument::load_from_path(&path) {
            Ok(document) => {
                let needs_async_decode = is_heif(&path);
                self.viewer.replace_document(document);
                self.ui.error_message = None;
                self.ui.show_zoom_menu = false;
                self.ui.is_panning = false;
                self.ui.context_menu_position = None;
                self.ui.last_mouse_position = None;

                self.navigation = FolderNavigation::scan(&path).unwrap_or_default();
                self.loads.sync_catalog(self.navigation.paths());
                self.window_title = self.image_name();
                if let Some(window) = window {
                    window.set_window_title(&self.window_title);
                }

                if let Some(cached) = self.loads.take_cached(&path) {
                    if let Some(document) = self.viewer.document_mut() {
                        document.cached_image = Some(cached);
                    }
                }

                if needs_async_decode
                    && self
                        .viewer
                        .document()
                        .and_then(|document| document.cached_image.as_ref())
                        .is_none()
                {
                    self.loads.begin_decode(generation);
                    let heif_bytes = self
                        .viewer
                        .document_mut()
                        .and_then(|document| document.heif_bytes.take());
                    self.start_current_decode(path.clone(), generation, heif_bytes, cx);
                }

                self.start_preload_adjacent(cx);
            }
            Err(error) => {
                self.ui.error_message = Some(format_load_error(&error));
                self.ui.is_panning = false;
                self.ui.context_menu_position = None;
                self.ui.last_mouse_position = None;
                self.ui.show_zoom_menu = false;
            }
        }
    }

    fn start_current_decode(
        &mut self,
        path: PathBuf,
        generation: u64,
        heif_bytes: Option<Vec<u8>>,
        cx: &mut Context<Self>,
    ) {
        let decode_path = path.clone();
        cx.spawn(async move |this, cx| {
            let cached = cx
                .background_executor()
                .spawn(async move {
                    heif_bytes
                        .or_else(|| std::fs::read(decode_path).ok())
                        .and_then(|bytes| lumia_core::decode_heic_to_png(&bytes).ok())
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                if !this.loads.finish_decode(generation)
                    || this.image_path() != Some(path.as_path())
                {
                    return;
                }
                if let Some(document) = this.viewer.document_mut() {
                    document.cached_image = cached;
                }
                if this.viewer.rotation_quarter_turns() != 0 {
                    this.rebuild_rotated_image();
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

        for target_path in adjacent.into_iter().filter(|path| is_heif(path)) {
            let Some(catalog_generation) = self.loads.queue_preload(target_path.clone()) else {
                continue;
            };
            let decode_path = target_path.clone();
            cx.spawn(async move |this, cx| {
                let cached = cx
                    .background_executor()
                    .spawn(async move {
                        std::fs::read(decode_path)
                            .ok()
                            .and_then(|bytes| lumia_core::decode_heic_to_png(&bytes).ok())
                    })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    if this
                        .loads
                        .complete_preload(target_path, catalog_generation, cached)
                    {
                        cx.notify();
                    }
                });
            })
            .detach();
        }
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
        let viewport_size = window.viewport_size();
        let available_width = f32::from(viewport_size.width).max(1.0);
        let available_height = f32::from(viewport_size.height).max(1.0);
        let image_width = image_width as f32;
        let image_height = image_height as f32;
        let fit_scale = (available_width / image_width)
            .min(available_height / image_height)
            .min(1.0);
        let scale = fit_scale * self.viewer.viewport().zoom;
        Some((image_width * scale, image_height * scale))
    }
}

fn is_heif(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("heic") || extension.eq_ignore_ascii_case("heif")
        })
}
