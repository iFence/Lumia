use std::sync::Arc;
use std::time::Duration;

use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, rgb, AnyElement, Context, Focusable, InteractiveElement, IntoElement, ParentElement,
    Point, Styled, Window,
};
use lumia_core::Annotation;
use lumia_plugin_api::{CanvasOperation, CanvasOperationCommittedParams, EmptyResult};

use crate::app::LumiaApp;
use crate::i18n::{tr, TextKey};
use crate::palette::Palette;
use crate::plugin_state::ActiveToolSettings;
use crate::ui_state::AnnotationDrag;

/// Minimum rectangle size in source pixels; smaller drags are treated as a
/// mis-click and discarded.
const MIN_RECTANGLE_SIZE: f32 = 4.0;

/// The image's display rect and the pointer mapping from display-local to
/// source-image coordinates, recomputed for each pointer event.
struct AnnotationGeometry {
    left: f32,
    top: f32,
    display_width: f32,
    display_height: f32,
    source_width: f32,
    source_height: f32,
}

impl LumiaApp {
    pub(crate) fn render_annotation_overlay(
        &self,
        display_size: Option<(f32, f32)>,
        palette: Palette,
    ) -> Option<AnyElement> {
        self.plugins.active.as_ref()?;
        (!self.loads.is_transitioning()).then_some(())?;
        let (display_width, _) = display_size?;
        let (source_width, _) = self.viewer.display_dimensions()?;
        let scale = display_width / source_width.max(1) as f32;

        let hint = self.annotations.items().is_empty()
            && self.ui.pending_text_point.is_none()
            && self.ui.annotation_drag.is_none();
        Some(
            div()
                .id("plugin-canvas-overlay")
                .absolute()
                .inset_0()
                .children(
                    self.annotations
                        .items()
                        .iter()
                        .map(|annotation| self.render_annotation(annotation, scale)),
                )
                .children(self.ui.pending_text_point.map(|(x, y)| {
                    div()
                        .absolute()
                        .left(px(x * scale - 8.0))
                        .top(px(y * scale - 8.0))
                        .size(px(16.0))
                        .rounded_full()
                        .border_2()
                        .border_color(rgb(palette.accent))
                }))
                .children(self.ui.annotation_drag.map(|drag| {
                    let left = drag.start_x.min(drag.current_x);
                    let top = drag.start_y.min(drag.current_y);
                    let width = (drag.start_x - drag.current_x).abs();
                    let height = (drag.start_y - drag.current_y).abs();
                    div()
                        .absolute()
                        .left(px(left))
                        .top(px(top))
                        .w(px(width))
                        .h(px(height))
                        .border_1()
                        .border_dashed()
                        .border_color(rgb(palette.accent))
                }))
                .when(hint, |overlay| {
                    overlay.child(
                        div()
                            .absolute()
                            .left_0()
                            .right_0()
                            .top_0()
                            .py_2()
                            .text_center()
                            .text_xs()
                            .text_color(rgb(palette.muted_text))
                            .child(tr(self.settings.language, TextKey::AnnotationPlaceHint)),
                    )
                })
                .into_any_element(),
        )
    }

    fn render_annotation(&self, annotation: &Annotation, scale: f32) -> AnyElement {
        match annotation {
            Annotation::Text {
                text,
                x,
                y,
                font_size,
                color,
                opacity,
            } => div()
                .absolute()
                .left(px(x * scale))
                .top(px(y * scale))
                .text_size(px(font_size * scale))
                .text_color(rgb(*color))
                .opacity(*opacity)
                .child(text.clone())
                .into_any_element(),
            Annotation::Rectangle {
                x,
                y,
                width,
                height,
                stroke_width,
                color,
                opacity,
            } => {
                let stroke = (stroke_width * scale).max(1.0);
                let x = x * scale;
                let y = y * scale;
                let width = width * scale;
                let height = height * scale;
                let bar = |left: f32, top: f32, width: f32, height: f32| {
                    div()
                        .absolute()
                        .left(px(left))
                        .top(px(top))
                        .w(px(width))
                        .h(px(height))
                        .bg(rgb(*color))
                };
                div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .opacity(*opacity)
                    .child(bar(0.0, 0.0, width, stroke))
                    .child(bar(0.0, height - stroke, width, stroke))
                    .child(bar(0.0, 0.0, stroke, height))
                    .child(bar(width - stroke, 0.0, stroke, height))
                    .into_any_element()
            }
            Annotation::Step {
                number,
                x,
                y,
                size,
                color,
                opacity,
            } => {
                let diameter = size * scale;
                div()
                    .absolute()
                    .left(px(x * scale - diameter / 2.0))
                    .top(px(y * scale - diameter / 2.0))
                    .size(px(diameter))
                    .rounded_full()
                    .bg(rgb(*color))
                    .opacity(*opacity)
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(px((diameter * 0.5).max(10.0)))
                    .text_color(rgb(0xffffff))
                    .child(number.to_string())
                    .into_any_element()
            }
        }
    }

    /// Dispatches an annotation click to the active tool: records a pending
    /// text point, begins a rectangle drag, or places a numbered step badge.
    /// Returns false when the pointer misses the image so the caller can pan.
    pub(crate) fn handle_annotation_mouse_down(
        &mut self,
        position: Point<gpui::Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(geometry) = self.annotation_geometry(window) else {
            return false;
        };
        let pointer_x = f32::from(position.x);
        let pointer_y = f32::from(position.y);
        if pointer_x < geometry.left
            || pointer_x > geometry.left + geometry.display_width
            || pointer_y < geometry.top
            || pointer_y > geometry.top + geometry.display_height
        {
            return false;
        }
        let local_x = pointer_x - geometry.left;
        let local_y = pointer_y - geometry.top;
        let x = (local_x / geometry.display_width * geometry.source_width)
            .clamp(0.0, geometry.source_width);
        let y = (local_y / geometry.display_height * geometry.source_height)
            .clamp(0.0, geometry.source_height);

        match self.plugins.active_tool_settings() {
            Some(ActiveToolSettings::Text { .. }) => {
                self.ui.pending_text_point = Some((x, y));
                if let Some(input) = self.annotation_text_input.clone() {
                    input.focus_handle(cx).focus(window, cx);
                }
                cx.notify();
                true
            }
            Some(ActiveToolSettings::Rectangle { .. }) => {
                self.ui.annotation_drag = Some(AnnotationDrag {
                    start_x: local_x,
                    start_y: local_y,
                    current_x: local_x,
                    current_y: local_y,
                });
                cx.notify();
                true
            }
            Some(ActiveToolSettings::NumberedStep {
                size,
                color,
                opacity,
            }) => {
                let number = self.annotations.next_step_number();
                self.annotations.place(Annotation::Step {
                    number,
                    x,
                    y,
                    size,
                    color,
                    opacity,
                });
                self.notify_canvas_operation(
                    CanvasOperation::StepPlaced {
                        number,
                        x,
                        y,
                        size,
                        color: format!("#{:06x}", color),
                        opacity,
                    },
                    cx,
                );
                cx.notify();
                true
            }
            None => false,
        }
    }

    pub(crate) fn update_annotation_drag(
        &mut self,
        position: Point<gpui::Pixels>,
        window: &Window,
    ) {
        let Some(geometry) = self.annotation_geometry(window) else {
            return;
        };
        if let Some(drag) = self.ui.annotation_drag.as_mut() {
            drag.current_x = f32::from(position.x) - geometry.left;
            drag.current_y = f32::from(position.y) - geometry.top;
        }
    }

    pub(crate) fn commit_annotation_rect(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(drag) = self.ui.annotation_drag else {
            return;
        };
        let Some(geometry) = self.annotation_geometry(window) else {
            return;
        };
        self.ui.annotation_drag = None;

        let left = drag.start_x.min(drag.current_x);
        let top = drag.start_y.min(drag.current_y);
        let width = (drag.start_x - drag.current_x).abs();
        let height = (drag.start_y - drag.current_y).abs();
        let x = (left / geometry.display_width * geometry.source_width)
            .clamp(0.0, geometry.source_width);
        let y = (top / geometry.display_height * geometry.source_height)
            .clamp(0.0, geometry.source_height);
        let width = (width / geometry.display_width * geometry.source_width)
            .clamp(0.0, geometry.source_width - x);
        let height = (height / geometry.display_height * geometry.source_height)
            .clamp(0.0, geometry.source_height - y);

        if width < MIN_RECTANGLE_SIZE || height < MIN_RECTANGLE_SIZE {
            cx.notify();
            return;
        }
        let Some(ActiveToolSettings::Rectangle {
            stroke_width,
            color,
            opacity,
        }) = self.plugins.active_tool_settings()
        else {
            cx.notify();
            return;
        };
        self.annotations.place(Annotation::Rectangle {
            x,
            y,
            width,
            height,
            stroke_width,
            color,
            opacity,
        });
        self.notify_canvas_operation(
            CanvasOperation::RectanglePlaced {
                x,
                y,
                width,
                height,
                stroke_width,
                color: format!("#{:06x}", color),
                opacity,
            },
            cx,
        );
        cx.notify();
    }

    pub(crate) fn handle_plugin_button(
        &mut self,
        control_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match control_id {
            "undo" => {
                if self.annotations.undo() {
                    self.notify_canvas_operation(CanvasOperation::Undo, cx);
                }
            }
            "redo" => {
                if self.annotations.redo() {
                    self.notify_canvas_operation(CanvasOperation::Redo, cx);
                }
            }
            "clear" => {
                if self.annotations.clear() {
                    self.notify_canvas_operation(CanvasOperation::Cleared, cx);
                }
            }
            "export" => {
                self.export_annotation_copy(window, cx);
                return;
            }
            _ => {}
        }
        self.dispatch_plugin_ui_event(control_id.to_string(), lumia_plugin_api::UiValue::None, cx);
        cx.notify();
    }

    pub(crate) fn notify_canvas_operation(&self, operation: CanvasOperation, cx: &mut Context<Self>) {
        let Some(active) = self.plugins.active.as_ref() else {
            return;
        };
        let process = Arc::clone(&active.process);
        let session_id = active.session_id.clone();
        cx.background_executor()
            .spawn(async move {
                if let Ok(mut process) = process.lock() {
                    let _ = process.request_with_timeout::<_, EmptyResult>(
                        "canvas.operation_committed",
                        CanvasOperationCommittedParams {
                            session_id,
                            operation,
                        },
                        Duration::from_secs(5),
                    );
                }
            })
            .detach();
    }

    fn annotation_geometry(&self, window: &Window) -> Option<AnnotationGeometry> {
        let (display_width, display_height) = self.scaled_image_size(window)?;
        let (source_width, source_height) = self.viewer.display_dimensions()?;
        let (viewer_width, viewer_height) = self.viewer_available_size(window);
        Some(AnnotationGeometry {
            left: (viewer_width - display_width) / 2.0 + self.viewer.viewport().pan_x,
            top: (viewer_height - display_height) / 2.0 + self.viewer.viewport().pan_y,
            display_width,
            display_height,
            source_width: source_width as f32,
            source_height: source_height as f32,
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn image_coordinate_mapping_preserves_center() {
        let source_width = 1000.0;
        let display_width = 500.0;
        let pointer = 250.0;
        let mapped = pointer / display_width * source_width;
        assert_eq!(mapped, 500.0);
    }
}
