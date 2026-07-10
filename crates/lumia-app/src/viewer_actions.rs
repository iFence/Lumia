use gpui::{Context, Window};
use lumia_core::{
    load_cached_image_from_path, rotate_cached_image, supported_image_extensions, FitMode,
};

use crate::app::LumiaApp;
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

    pub(crate) fn zoom_in(&mut self, _: &ZoomIn, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_viewer_blocked() {
            return;
        }
        self.viewer.viewport_mut().zoom_in();
        self.ui.show_zoom_menu = false;
        cx.notify();
    }

    pub(crate) fn zoom_out(&mut self, _: &ZoomOut, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_viewer_blocked() {
            return;
        }
        self.viewer.viewport_mut().zoom_out();
        self.ui.show_zoom_menu = false;
        cx.notify();
    }

    pub(crate) fn zoom_fit(&mut self, _: &ZoomFit, _: &mut Window, cx: &mut Context<Self>) {
        if !self.is_viewer_blocked() {
            self.reset_fit(cx);
        }
    }

    pub(crate) fn reset_fit(&mut self, cx: &mut Context<Self>) {
        self.viewer.viewport_mut().reset_fit();
        self.ui.show_zoom_menu = false;
        cx.notify();
    }

    pub(crate) fn set_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        if self.is_viewer_blocked() || !self.viewer.has_document() {
            return;
        }
        self.viewer.viewport_mut().set_zoom(zoom);
        self.ui.show_zoom_menu = false;
        cx.notify();
    }

    pub(crate) fn toggle_fit_or_actual_size(&mut self, cx: &mut Context<Self>) {
        if self.is_viewer_blocked() || !self.viewer.has_document() {
            return;
        }
        if self.viewer.viewport().fit_mode == FitMode::FitToWindow {
            self.viewer.viewport_mut().set_zoom(1.0);
        } else {
            self.viewer.viewport_mut().reset_fit();
        }
        self.ui.show_zoom_menu = false;
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
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.rotate_display(1, cx);
    }

    pub(crate) fn rotate_counter_clockwise(
        &mut self,
        _: &RotateCounterClockwise,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.rotate_display(3, cx);
    }

    pub(crate) fn rotate_display(&mut self, quarter_turns: u8, cx: &mut Context<Self>) {
        if self.is_viewer_blocked() || !self.viewer.has_document() {
            return;
        }
        self.viewer.rotate_by(quarter_turns);
        self.ui.show_zoom_menu = false;
        self.rebuild_rotated_image();
        cx.notify();
    }

    pub(crate) fn rebuild_rotated_image(&mut self) {
        self.viewer.set_rotated_image(None);
        let turns = self.viewer.rotation_quarter_turns();
        if turns == 0 {
            return;
        }

        let cached = self
            .viewer
            .document()
            .and_then(|document| document.cached_image.clone())
            .or_else(|| {
                self.image_path()
                    .and_then(|path| load_cached_image_from_path(path).ok())
            });
        let rotated = cached
            .as_ref()
            .and_then(|image| rotate_cached_image(image, turns).ok());
        self.viewer.set_rotated_image(rotated);
    }
}
