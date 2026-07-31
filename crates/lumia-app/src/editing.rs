use std::path::PathBuf;

use gpui::{App, AppContext, Context, Entity, Subscription, Window};
use gpui_component::input::{InputEvent, InputState};
use lumia_core::{CropRect, ImageEditPolicy, ViewportState};

use crate::app::LumiaApp;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditMode {
    Crop,
    Resize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CropAspect {
    Free,
    Original,
    Square,
    FourThree,
    SixteenNine,
}

impl CropAspect {
    pub(crate) fn ratio(self, width: u32, height: u32) -> Option<f32> {
        match self {
            Self::Free => None,
            Self::Original => Some(width as f32 / height.max(1) as f32),
            Self::Square => Some(1.0),
            Self::FourThree => Some(4.0 / 3.0),
            Self::SixteenNine => Some(16.0 / 9.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CropDragKind {
    Move,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct CropDrag {
    pub(crate) kind: CropDragKind,
    pub(crate) start_x: f32,
    pub(crate) start_y: f32,
    pub(crate) start_rect: CropRect,
}

pub(crate) struct EditState {
    pub(crate) mode: Option<EditMode>,
    pub(crate) show_menu: bool,
    pub(crate) source_width: u32,
    pub(crate) source_height: u32,
    pub(crate) rotation_quarter_turns: u8,
    pub(crate) crop_rect: CropRect,
    pub(crate) crop_aspect: CropAspect,
    pub(crate) crop_drag: Option<CropDrag>,
    pub(crate) resize_width: u32,
    pub(crate) resize_height: u32,
    pub(crate) lock_aspect: bool,
    pub(crate) width_input: Option<Entity<InputState>>,
    pub(crate) height_input: Option<Entity<InputState>>,
    pub(crate) input_subscriptions: Vec<Subscription>,
    pub(crate) saved_viewport: Option<ViewportState>,
    pub(crate) exporting: bool,
    pub(crate) feedback: Option<Result<PathBuf, String>>,
    pub(crate) generation: u64,
}

impl Default for EditState {
    fn default() -> Self {
        Self {
            mode: None,
            show_menu: false,
            source_width: 0,
            source_height: 0,
            rotation_quarter_turns: 0,
            crop_rect: CropRect::new(0, 0, 0, 0),
            crop_aspect: CropAspect::Free,
            crop_drag: None,
            resize_width: 0,
            resize_height: 0,
            lock_aspect: true,
            width_input: None,
            height_input: None,
            input_subscriptions: Vec::new(),
            saved_viewport: None,
            exporting: false,
            feedback: None,
            generation: 0,
        }
    }
}

impl LumiaApp {
    pub(crate) fn editing_unavailable_reason(&self) -> Option<&'static str> {
        let Some(path) = self.image_path() else {
            return Some("no-image");
        };
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if matches!(extension.as_str(), "gif" | "svg" | "psd" | "psb")
            || lumia_core::is_raw_image_extension(&extension)
        {
            return Some("format");
        }
        if self.large_image.is_active(path) {
            return Some("large");
        }
        if self.loads.is_decoding() {
            return Some("loading");
        }
        let Some(expected) = self.viewer.display_dimensions() else {
            return Some("preview");
        };
        let Some(actual) = self
            .loads
            .display_image(self.viewer.rotation_quarter_turns())
            .map(|image| image.dimensions())
        else {
            return Some("preview");
        };
        (actual != expected).then_some("preview")
    }

    pub(crate) fn can_edit_current_image(&self) -> bool {
        self.viewer.has_document() && self.editing_unavailable_reason().is_none()
    }

    pub(crate) fn open_edit_mode(
        &mut self,
        mode: EditMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.stop_slideshow(cx);
        if !self.can_edit_current_image() {
            return;
        }
        self.close_plugin_session(cx);
        let Some((width, height)) = self.viewer.display_dimensions() else {
            return;
        };
        let saved_viewport = self
            .editing
            .saved_viewport
            .unwrap_or(*self.viewer.viewport());
        self.editing.input_subscriptions.clear();
        self.editing.mode = Some(mode);
        self.editing.show_menu = false;
        self.editing.source_width = width;
        self.editing.source_height = height;
        self.editing.rotation_quarter_turns = self.viewer.rotation_quarter_turns();
        self.editing.crop_rect = CropRect::new(0, 0, width, height);
        self.editing.crop_aspect = CropAspect::Free;
        self.editing.crop_drag = None;
        self.editing.resize_width = width;
        self.editing.resize_height = height;
        self.editing.lock_aspect = true;
        self.editing.saved_viewport = Some(saved_viewport);
        self.editing.exporting = false;
        self.editing.feedback = None;
        self.editing.generation = self.editing.generation.wrapping_add(1);
        self.viewer.viewport_mut().reset_fit();
        if mode == EditMode::Resize {
            self.initialize_resize_inputs(window, cx);
        } else {
            self.editing.width_input = None;
            self.editing.height_input = None;
        }
        self.ui.context_menu_position = None;
        self.ui.show_zoom_menu = false;
        cx.notify();
    }

    fn initialize_resize_inputs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let width = self.editing.resize_width;
        let height = self.editing.resize_height;
        let width_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(width.to_string())
                .placeholder("Width")
        });
        let height_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(height.to_string())
                .placeholder("Height")
        });
        self.editing.input_subscriptions = vec![
            cx.subscribe_in(&width_input, window, Self::handle_resize_input),
            cx.subscribe_in(&height_input, window, Self::handle_resize_input),
        ];
        self.editing.width_input = Some(width_input);
        self.editing.height_input = Some(height_input);
    }

    fn handle_resize_input(
        &mut self,
        input: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(event, InputEvent::Change) {
            return;
        }
        let Ok(value) = input.read(cx).value().parse::<u32>() else {
            cx.notify();
            return;
        };
        if value == 0 {
            cx.notify();
            return;
        }
        let is_width = self
            .editing
            .width_input
            .as_ref()
            .is_some_and(|width| width == input);
        if is_width {
            self.editing.resize_width = value;
            if self.editing.lock_aspect {
                let height =
                    linked_dimension(value, self.editing.source_width, self.editing.source_height);
                self.editing.resize_height = height;
                if let Some(height_input) = self.editing.height_input.clone() {
                    height_input.update(cx, |state, cx| {
                        state.set_value(height.to_string(), window, cx)
                    });
                }
            }
        } else {
            self.editing.resize_height = value;
            if self.editing.lock_aspect {
                let width =
                    linked_dimension(value, self.editing.source_height, self.editing.source_width);
                self.editing.resize_width = width;
                if let Some(width_input) = self.editing.width_input.clone() {
                    width_input.update(cx, |state, cx| {
                        state.set_value(width.to_string(), window, cx)
                    });
                }
            }
        }
        self.editing.feedback = None;
        cx.notify();
    }

    pub(crate) fn toggle_resize_aspect_lock(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editing.lock_aspect = !self.editing.lock_aspect;
        if self.editing.lock_aspect {
            let height = linked_dimension(
                self.editing.resize_width,
                self.editing.source_width,
                self.editing.source_height,
            );
            self.editing.resize_height = height;
            if let Some(input) = self.editing.height_input.clone() {
                input.update(cx, |state, cx| {
                    state.set_value(height.to_string(), window, cx)
                });
            }
        }
        self.editing.feedback = None;
        cx.notify();
    }

    pub(crate) fn reset_resize(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editing.resize_width = self.editing.source_width;
        self.editing.resize_height = self.editing.source_height;
        self.editing.lock_aspect = true;
        if let Some(input) = self.editing.width_input.clone() {
            let value = self.editing.resize_width.to_string();
            input.update(cx, |state, cx| state.set_value(value, window, cx));
        }
        if let Some(input) = self.editing.height_input.clone() {
            let value = self.editing.resize_height.to_string();
            input.update(cx, |state, cx| state.set_value(value, window, cx));
        }
        self.editing.feedback = None;
        cx.notify();
    }

    pub(crate) fn select_crop_aspect(&mut self, aspect: CropAspect, cx: &mut Context<Self>) {
        self.editing.crop_aspect = aspect;
        if let Some(ratio) = aspect.ratio(self.editing.source_width, self.editing.source_height) {
            self.editing.crop_rect = fitted_aspect_rect(
                self.editing.crop_rect,
                self.editing.source_width,
                self.editing.source_height,
                ratio,
            );
        }
        self.editing.feedback = None;
        cx.notify();
    }

    pub(crate) fn reset_crop(&mut self, cx: &mut Context<Self>) {
        self.editing.crop_rect =
            CropRect::new(0, 0, self.editing.source_width, self.editing.source_height);
        self.editing.crop_aspect = CropAspect::Free;
        self.editing.feedback = None;
        cx.notify();
    }

    pub(crate) fn close_edit_session(&mut self, restore_viewport: bool, cx: &mut Context<Self>) {
        if restore_viewport {
            if let Some(viewport) = self.editing.saved_viewport {
                *self.viewer.viewport_mut() = viewport;
            }
        }
        let generation = self.editing.generation.wrapping_add(1);
        self.editing = EditState {
            generation,
            ..EditState::default()
        };
        cx.notify();
    }

    pub(crate) fn current_resize_values(&self, cx: &App) -> Option<(u32, u32)> {
        let width = self
            .editing
            .width_input
            .as_ref()?
            .read(cx)
            .value()
            .parse()
            .ok()?;
        let height = self
            .editing
            .height_input
            .as_ref()?
            .read(cx)
            .value()
            .parse()
            .ok()?;
        Some((width, height))
    }

    pub(crate) fn resize_is_valid(&self, cx: &App) -> bool {
        self.current_resize_values(cx)
            .is_some_and(|(width, height)| valid_output_dimensions(width, height))
    }
}

fn linked_dimension(value: u32, source_axis: u32, linked_axis: u32) -> u32 {
    ((u64::from(value) * u64::from(linked_axis) + u64::from(source_axis) / 2)
        / u64::from(source_axis.max(1)))
    .clamp(1, u64::from(u32::MAX)) as u32
}

fn valid_output_dimensions(width: u32, height: u32) -> bool {
    width > 0
        && height > 0
        && u64::from(width)
            .checked_mul(u64::from(height))
            .and_then(|pixels| pixels.checked_mul(4))
            .is_some_and(|bytes| bytes <= ImageEditPolicy::default().max_output_bytes)
}

fn fitted_aspect_rect(
    current: CropRect,
    image_width: u32,
    image_height: u32,
    ratio: f32,
) -> CropRect {
    let center_x = current.x as f32 + current.width as f32 / 2.0;
    let center_y = current.y as f32 + current.height as f32 / 2.0;
    let max_width = (center_x.min(image_width as f32 - center_x) * 2.0).max(1.0);
    let max_height = (center_y.min(image_height as f32 - center_y) * 2.0).max(1.0);
    let (width, height) = if max_width / max_height > ratio {
        (max_height * ratio, max_height)
    } else {
        (max_width, max_width / ratio)
    };
    let width = width.round().max(1.0) as u32;
    let height = height.round().max(1.0) as u32;
    CropRect::new(
        (center_x - width as f32 / 2.0)
            .round()
            .clamp(0.0, image_width.saturating_sub(width) as f32) as u32,
        (center_y - height as f32 / 2.0)
            .round()
            .clamp(0.0, image_height.saturating_sub(height) as f32) as u32,
        width.min(image_width),
        height.min(image_height),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linked_dimensions_round_and_never_reach_zero() {
        assert_eq!(linked_dimension(1000, 1920, 1080), 563);
        assert_eq!(linked_dimension(1, 10_000, 1), 1);
    }

    #[test]
    fn aspect_rect_stays_centered_and_bounded() {
        let rect = fitted_aspect_rect(CropRect::new(0, 0, 1600, 900), 1600, 900, 1.0);
        assert_eq!(rect, CropRect::new(350, 0, 900, 900));
    }
}
