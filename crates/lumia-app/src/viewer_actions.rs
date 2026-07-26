use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use gpui::{AppContext, Context, Focusable, ParentElement, Window};
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
use crate::{OpenFile, RotateClockwise, RotateCounterClockwise, ZoomFit, ZoomIn, ZoomOut};

impl LumiaApp {
    pub(crate) fn open_file(&mut self, _: &OpenFile, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_viewer_blocked() {
            self.open_file_dialog(cx, Some(window));
        }
    }

    pub(crate) fn open_file_dialog(&mut self, cx: &mut Context<Self>, window: Option<&mut Window>) {
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
        self.prepare_manual_zoom(window);
        self.viewer.viewport_mut().zoom_in();
        self.ui.show_zoom_menu = false;
        self.refresh_large_image_tiles(window, cx);
        cx.notify();
    }

    pub(crate) fn zoom_out_view(&mut self, window: &Window, cx: &mut Context<Self>) {
        self.prepare_manual_zoom(window);
        self.viewer.viewport_mut().zoom_out();
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
        cx.notify();
    }

    pub(crate) fn rotate_clockwise(
        &mut self,
        _: &RotateClockwise,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.rotate_display(1, window, cx);
    }

    pub(crate) fn rotate_counter_clockwise(
        &mut self,
        _: &RotateCounterClockwise,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.rotate_display(3, window, cx);
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
        self.viewer.rotate_by(quarter_turns);
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
        self.loads.set_rotated_image(None);
        let turns = self.viewer.rotation_quarter_turns();
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
        self.loads.set_rotated_image(rotated);
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
