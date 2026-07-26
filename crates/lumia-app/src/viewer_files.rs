use std::path::Path;

use gpui::{Context, Window};
use lumia_core::{FitMode, ViewportState};

use crate::app::LumiaApp;
use crate::{NextImage, PreviousImage};
use crate::{EDIT_PANEL_WIDTH, STATUS_BAR_HEIGHT};

impl LumiaApp {
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
        let (available_width, available_height) = self.viewer_available_size(window);
        Some(display_scale(
            image_width,
            image_height,
            available_width,
            available_height,
            self.viewer.viewport(),
        ))
    }

    pub(crate) fn viewer_available_size(&self, window: &Window) -> (f32, f32) {
        let viewport_size = window.viewport_size();
        let panel_width = if self.editing.mode.is_some() {
            EDIT_PANEL_WIDTH
        } else {
            0.0
        };
        available_viewer_size(
            f32::from(viewport_size.width),
            f32::from(viewport_size.height),
            panel_width,
            self.ui.status_bar_locked,
        )
    }
}

fn available_viewer_size(
    viewport_width: f32,
    viewport_height: f32,
    panel_width: f32,
    status_bar_locked: bool,
) -> (f32, f32) {
    let status_bar_height = if status_bar_locked {
        STATUS_BAR_HEIGHT
    } else {
        0.0
    };
    (
        (viewport_width - panel_width).max(1.0),
        (viewport_height - status_bar_height).max(1.0),
    )
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

    #[test]
    fn locked_status_bar_reserves_viewer_height() {
        assert_eq!(
            available_viewer_size(1200.0, 800.0, 0.0, false),
            (1200.0, 800.0)
        );
        assert_eq!(
            available_viewer_size(1200.0, 800.0, EDIT_PANEL_WIDTH, true),
            (880.0, 764.0)
        );
    }
}
