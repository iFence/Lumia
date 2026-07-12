use gpui::{Context, Window};
use lumia_core::{
    load_decoded_image_from_path, rotate_bgra8, rotate_decoded_image, supported_image_extensions,
    FitMode,
};

use crate::app::LumiaApp;
use crate::load_state::PreparedImage;
use crate::{OpenFile, RotateClockwise, RotateCounterClockwise, ZoomFit, ZoomIn, ZoomOut};

impl LumiaApp {
    pub(crate) fn open_file(&mut self, _: &OpenFile, window: &mut Window, cx: &mut Context<Self>) {
        if !self.is_viewer_blocked() {
            self.open_file_dialog(cx, Some(window));
        }
    }

    pub(crate) fn open_file_dialog(&mut self, cx: &mut Context<Self>, window: Option<&mut Window>) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Images", supported_image_extensions())
            .pick_file()
        {
            self.load_image(path, window, cx);
            cx.notify();
        }
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
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_viewer_blocked() || !self.viewer.has_document() {
            return;
        }
        self.viewer.rotate_by(quarter_turns);
        self.ui.show_zoom_menu = false;
        self.rebuild_rotated_image();
        self.refresh_large_image_tiles(window, cx);
        cx.notify();
    }

    pub(crate) fn rebuild_rotated_image(&mut self) {
        self.loads.set_rotated_image(None);
        let turns = self.viewer.rotation_quarter_turns();
        if turns == 0 {
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
    }
}
