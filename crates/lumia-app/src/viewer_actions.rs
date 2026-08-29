use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use gpui::{AppContext, Context, Focusable, ParentElement, Pixels, Point, Window};
use gpui_component::dialog::DialogButtonProps;
use gpui_component::input::{Input, InputState};
use gpui_component::WindowExt;
use http_client::{AsyncBody, HttpClient};
use lumia_core::{
    load_decoded_image_from_path, rotate_bgra8, rotate_decoded_image, supported_image_extensions,
    FitMode,
};

use crate::app::LumiaApp;
use crate::i18n::{tr, TextKey};
use crate::load_state::PreparedImage;
use crate::{
    CloseComparison, CompareImages, OpenFile, RotateClockwise, RotateCounterClockwise,
    ToggleComparisonTarget, ZoomFit, ZoomIn, ZoomOut,
};

impl LumiaApp {
    pub(crate) fn compare_images(
        &mut self,
        _: &CompareImages,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_viewer_blocked() || self.image_path().is_none() {
            return;
        }
        let handle = self.self_handle.clone();
        cx.spawn(async move |_, cx| {
            let picked = cx
                .background_executor()
                .spawn(async move {
                    rfd::FileDialog::new()
                        .add_filter("Images", supported_image_extensions())
                        .pick_file()
                })
                .await;
            if let Some(path) = picked {
                let _ = handle.update(cx, |this, cx| this.start_comparison(path, cx));
            }
        })
        .detach();
    }

    pub(crate) fn toggle_comparison_target_action(
        &mut self,
        _: &ToggleComparisonTarget,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_comparison_target(cx);
    }

    pub(crate) fn close_comparison_action(
        &mut self,
        _: &CloseComparison,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_comparison(cx);
    }
}
impl LumiaApp {
    pub(crate) fn open_file(&mut self, _: &OpenFile, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_viewer_blocked() {
            self.open_file_dialog(cx, Some(window));
        }
    }

    pub(crate) fn open_file_dialog(&mut self, cx: &mut Context<Self>, window: Option<&mut Window>) {
        self.stop_slideshow(cx);
        // Capture the window handle so we can still update the title after the
        // (now asynchronous) dialog resolves. `pick_file()` runs a blocking
        // `NSOpenPanel::runModal` on the main thread; invoking it directly from
        // a GPUI event handler nests a modal run loop inside GPUI's own run
        // loop, which crashes on macOS. Offloading it to the background
        // executor lets rfd dispatch the panel onto the main run loop safely.
        let window_handle = window.map(|window| window.window_handle());
        let handle = self.self_handle.clone();
        cx.spawn(async move |_this, cx| {
            let picked = cx
                .background_executor()
                .spawn(async move {
                    rfd::FileDialog::new()
                        .add_filter("Images", supported_image_extensions())
                        .pick_file()
                })
                .await;
            let Some(path) = picked else { return };
            let _ = handle.update(cx, |this, cx| {
                this.load_image(path, None, cx);
                cx.notify();
            });
            if let Some(window_handle) = window_handle {
                let title = handle
                    .update(cx, |this, _| this.window_title.clone())
                    .unwrap_or_default();
                let _ = window_handle.update(cx, |_, window, _| {
                    window.set_window_title(&title);
                });
            }
        })
        .detach();
    }

    pub(crate) fn open_url_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_viewer_blocked() {
            return;
        }
        self.stop_slideshow(cx);
        self.ui.context_menu_position = None;
        cx.notify();
        let language = self.settings.language;
        let url_input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(tr(language, TextKey::OpenUrlPlaceholder))
        });
        let self_handle = self.self_handle.clone();
        let url_input_for_ok = url_input.clone();
        let url_input_for_content = url_input.clone();
        let url_input_focus = url_input.focus_handle(cx);
        window.open_dialog(cx, move |dialog, _, _| {
            let url_input_for_ok = url_input_for_ok.clone();
            let url_input_for_content = url_input_for_content.clone();
            dialog
                .title(tr(language, TextKey::OpenUrlDialogTitle))
                .close_button(true)
                .overlay_closable(true)
                .button_props(
                    DialogButtonProps::default()
                        .show_cancel(true)
                        .ok_text(tr(language, TextKey::Confirm))
                        .cancel_text(tr(language, TextKey::Cancel)),
                )
                .on_cancel(|_, _, _| true)
                .on_ok({
                    let url_input = url_input_for_ok;
                    let self_handle = self_handle.clone();
                    move |_, _window, cx| {
                        let url = url_input.read(cx).value().trim().to_string();
                        if !url.starts_with("http://") && !url.starts_with("https://") {
                            let _ = self_handle.update(cx, |this, cx| {
                                this.ui.error_message = Some(
                                    tr(this.settings.language, TextKey::OpenUrlInvalid).into(),
                                );
                                cx.notify();
                            });
                            return true;
                        }
                        let handle = self_handle.clone();
                        let client = cx.http_client();
                        cx.spawn(async move |cx| match download_image(&client, &url).await {
                            Ok(path) => {
                                let _ = handle.update(cx, |this, cx| {
                                    this.load_image(path, None, cx);
                                    cx.notify();
                                });
                            }
                            Err(error) => {
                                let _ = handle.update(cx, |this, cx| {
                                    this.ui.error_message = Some(
                                        format!(
                                            "{}: {error:#}",
                                            tr(
                                                this.settings.language,
                                                TextKey::OpenUrlDownloadFailed
                                            )
                                        )
                                        .into(),
                                    );
                                    cx.notify();
                                });
                            }
                        })
                        .detach();
                        true
                    }
                })
                .content(move |content, _, _| content.child(Input::new(&url_input_for_content)))
        });
        url_input_focus.focus(window, cx);
    }

    pub(crate) fn zoom_in(&mut self, _: &ZoomIn, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_viewer_blocked() {
            return;
        }
        self.zoom_in_view(window, cx);
    }

    pub(crate) fn zoom_out(&mut self, _: &ZoomOut, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_viewer_blocked() {
            return;
        }
        self.zoom_out_view(window, cx);
    }

    pub(crate) fn zoom_in_view(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.ui.show_zoom_menu = false;
        // Comparison mode drives two viewports at once, so it owns its own
        // stepped zoom. `compare_zoom_step` performs the FitToWindow
        // conversion that `prepare_manual_zoom` does for the single viewer.
        if self.comparison_active() {
            self.compare_zoom_in(window, cx);
            return;
        }
        self.prepare_manual_zoom(window);
        self.viewer.viewport_mut().zoom_in();
        self.refresh_large_image_tiles(window, cx);
        cx.notify();
    }

    pub(crate) fn zoom_out_view(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.ui.show_zoom_menu = false;
        if self.comparison_active() {
            self.compare_zoom_out(window, cx);
            return;
        }
        self.prepare_manual_zoom(window);
        self.viewer.viewport_mut().zoom_out();
        self.refresh_large_image_tiles(window, cx);
        cx.notify();
    }

    /// Scroll-wheel zoom anchored on the cursor position: the image point
    /// under the cursor stays fixed while the scale steps.
    ///
    /// The pre-zoom scale is derived from the displayed image frame's real
    /// on-screen bounds (captured each paint), and the post-zoom pan is set
    /// absolutely so anchoring never depends on recomputing layout analytically.
    pub(crate) fn scroll_zoom_at_cursor(
        &mut self,
        inward: bool,
        cursor: Point<Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_viewer_blocked() {
            return;
        }
        // Convert FitToWindow to an equivalent ActualSize zoom first so the
        // step multiplies the scale that is actually on screen.
        self.prepare_manual_zoom(window);
        let image_bounds = self.ui.viewer_image_bounds;
        let surface_center = self.ui.viewer_surface_bounds.map(|bounds| bounds.center());
        let dimensions = self.displayed_source_dimensions();
        {
            let viewport = self.viewer.viewport_mut();
            if inward {
                viewport.zoom_in();
            } else {
                viewport.zoom_out();
            }
        }
        // The renderer sizes the frame from this function's output, so gate on
        // it being known rather than trusting a letterboxed fallback element.
        if image_bounds.is_some()
            && surface_center.is_some()
            && self.scaled_image_size(window).is_some()
        {
            if let (Some(image_bounds), Some(center), Some(dimensions)) =
                (image_bounds, surface_center, dimensions)
            {
                let old_scale = f32::from(image_bounds.size.width) / dimensions.0 as f32;
                let new_scale = self.viewer.viewport().zoom;
                if old_scale > 1e-6 && old_scale.is_finite() && new_scale.is_finite() {
                    let cursor_x = f32::from(cursor.x);
                    let cursor_y = f32::from(cursor.y);
                    let anchor_x = (cursor_x - f32::from(image_bounds.origin.x)) / old_scale;
                    let anchor_y = (cursor_y - f32::from(image_bounds.origin.y)) / old_scale;
                    // Frame origin that keeps the anchored point under the
                    // cursor at the new scale.
                    let origin_x = cursor_x - anchor_x * new_scale;
                    let origin_y = cursor_y - anchor_y * new_scale;
                    // The renderer places the frame at
                    // center - dims*scale/2 + pan, so invert for the pan.
                    let pan_x =
                        origin_x - f32::from(center.x) + dimensions.0 as f32 * new_scale / 2.0;
                    let pan_y =
                        origin_y - f32::from(center.y) + dimensions.1 as f32 * new_scale / 2.0;
                    self.viewer.viewport_mut().set_pan(pan_x, pan_y);
                }
            }
        }
        self.ui.show_zoom_menu = false;
        self.refresh_large_image_tiles(window, cx);
        cx.notify();
    }

    fn prepare_manual_zoom(&mut self, window: &Window) {
        if self.viewer.viewport().fit_mode == FitMode::FitToWindow {
            if let Some(scale) = self.image_display_scale(window) {
                self.viewer.viewport_mut().set_zoom(scale);
            }
        }
    }

    pub(crate) fn zoom_fit(&mut self, _: &ZoomFit, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_viewer_blocked() {
            self.reset_fit(window, cx);
        }
    }

    pub(crate) fn reset_fit(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.viewer.viewport_mut().reset_fit();
        self.ui.show_zoom_menu = false;
        self.refresh_large_image_tiles(window, cx);
        cx.notify();
    }

    pub(crate) fn set_zoom(&mut self, zoom: f32, window: &Window, cx: &mut Context<Self>) {
        if self.is_viewer_blocked() || !self.viewer.has_document() {
            return;
        }
        self.viewer.viewport_mut().set_zoom(zoom);
        self.ui.show_zoom_menu = false;
        self.refresh_large_image_tiles(window, cx);
        cx.notify();
    }
    pub(crate) fn toggle_fit_or_actual_size(&mut self, window: &Window, cx: &mut Context<Self>) {
        if self.is_viewer_blocked() || !self.viewer.has_document() {
            return;
        }
        if self.viewer.viewport().fit_mode == FitMode::FitToWindow {
            self.viewer.viewport_mut().reset_actual_size();
        } else {
            self.viewer.viewport_mut().reset_fit();
        }
        self.ui.show_zoom_menu = false;
        self.refresh_large_image_tiles(window, cx);
        cx.notify();
    }

    pub(crate) fn toggle_zoom_menu(&mut self, cx: &mut Context<Self>) {
        if self.is_viewer_blocked() || !self.viewer.has_document() {
            return;
        }
        self.ui.show_zoom_menu = !self.ui.show_zoom_menu;
        self.editing.show_menu = false;
        cx.notify();
    }

    pub(crate) fn rotate_clockwise(
        &mut self,
        _: &RotateClockwise,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.comparison_active() {
            self.compare_rotate(1, window, cx);
        } else {
            self.rotate_display(1, window, cx);
        }
    }

    pub(crate) fn rotate_counter_clockwise(
        &mut self,
        _: &RotateCounterClockwise,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.comparison_active() {
            self.compare_rotate(3, window, cx);
        } else {
            self.rotate_display(3, window, cx);
        }
    }

    pub(crate) fn rotate_display(
        &mut self,
        quarter_turns: u8,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_viewer_blocked() || !self.viewer.has_document() {
            return;
        }
        let previous_dimensions = self.viewer.display_dimensions();
        self.viewer.rotate_by(quarter_turns);
        if let Some((width, height)) = previous_dimensions {
            self.annotations.rotate_by(quarter_turns, width, height);
        }
        self.ui.show_zoom_menu = false;
        self.rebuild_rotated_image(Some(window), cx);
        self.refresh_large_image_tiles(window, cx);
        cx.notify();
    }

    pub(crate) fn rebuild_rotated_image(
        &mut self,
        current_window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        let turns = self.viewer.rotation_quarter_turns();
        self.loads.set_rotated_image(None, turns);
        if turns == 0 {
            self.release_retired_images(current_window, cx);
            return;
        }

        let rotated = if let Some(image) = self.loads.current_image() {
            let (width, height) = image.dimensions();
            image
                .pixels_bgra8()
                .and_then(|pixels| rotate_bgra8(pixels, width, height, turns).ok())
        } else if self.loads.is_decoding() {
            // The progressive decoder will rebuild rotation when its preview
            // or full-resolution frame arrives. Avoid a synchronous HEIC
            // decode on the UI thread while that work is already in flight.
            None
        } else {
            self.image_path()
                .and_then(|path| load_decoded_image_from_path(path).ok())
                .and_then(|image| rotate_decoded_image(&image, turns).ok())
        }
        .map(PreparedImage::from_decoded);
        self.loads.set_rotated_image(rotated, turns);
        self.release_retired_images(current_window, cx);
    }
}

async fn download_image(client: &Arc<dyn HttpClient>, url: &str) -> anyhow::Result<PathBuf> {
    let response = client.get(url, AsyncBody::from(()), true).await?;
    if !response.status().is_success() {
        anyhow::bail!("http status {}", response.status());
    }
    let (_, mut body) = response.into_parts();
    let mut bytes = Vec::new();
    futures::io::AsyncReadExt::read_to_end(&mut body, &mut bytes).await?;

    let extension = extension_for_bytes(&bytes)
        .unwrap_or_else(|| extension_from_url(url).unwrap_or_else(|| "png".into()));
    let mut temp = std::env::temp_dir();
    temp.push(format!("lumia-url-{}.{}", std::process::id(), extension));
    let mut file = std::fs::File::create(&temp)?;
    file.write_all(&bytes)?;
    Ok(temp)
}

fn extension_from_url(url: &str) -> Option<String> {
    let path = url.split('?').next().unwrap_or(url);
    let name = path.rsplit('/').next()?;
    let extension = name.rsplit('.').next()?;
    if extension.is_empty() || extension == name {
        return None;
    }
    if lumia_core::is_supported_image_extension(extension) {
        Some(extension.to_ascii_lowercase())
    } else {
        None
    }
}

fn extension_for_bytes(bytes: &[u8]) -> Option<String> {
    match bytes {
        b if b.starts_with(b"\x89PNG") => Some("png".into()),
        b if b.len() >= 3 && b[0] == 0xFF && b[1] == 0xD8 && b[2] == 0xFF => Some("jpg".into()),
        b if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") => Some("gif".into()),
        b if b.starts_with(b"RIFF") && b.len() >= 12 && &b[8..12] == b"WEBP" => Some("webp".into()),
        b if b.starts_with(b"BM") => Some("bmp".into()),
        b if b.starts_with(b"%PDF") => Some("pdf".into()),
        _ => None,
    }
}
