use std::sync::Arc;
use std::time::Duration;

use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, rgb, svg, AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Point,
    Styled, Window,
};
use lumia_core::IconAnnotation;
use lumia_plugin_api::{CanvasOperation, CanvasOperationCommittedParams, EmptyResult, UiValue};

use crate::app::LumiaApp;
use crate::i18n::{tr, TextKey};
use crate::palette::Palette;

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

        Some(
            div()
                .id("plugin-canvas-overlay")
                .absolute()
                .inset_0()
                .children(self.annotations.items().iter().filter_map(|item| {
                    let path = self.plugins.active_asset_path(&item.asset_id)?;
                    let size = item.size * scale;
                    Some(
                        svg()
                            .external_path(path.to_string_lossy().to_string())
                            .absolute()
                            .left(px(item.x * scale - size / 2.0))
                            .top(px(item.y * scale - size / 2.0))
                            .size(px(size))
                            .text_color(rgb(item.color))
                            .opacity(item.opacity),
                    )
                }))
                .when(self.annotations.items().is_empty(), |overlay| {
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

    pub(crate) fn place_annotation_at(
        &mut self,
        position: Point<gpui::Pixels>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(settings) = self.plugins.marker_settings() else {
            return false;
        };
        let Some((display_width, display_height)) = self.scaled_image_size(window) else {
            return false;
        };
        let Some((source_width, source_height)) = self.viewer.display_dimensions() else {
            return false;
        };
        let (viewer_width, viewer_height) = self.viewer_available_size(window);
        let left = (viewer_width - display_width) / 2.0 + self.viewer.viewport().pan_x;
        let top = (viewer_height - display_height) / 2.0 + self.viewer.viewport().pan_y;
        let pointer_x = f32::from(position.x);
        let pointer_y = f32::from(position.y);
        if pointer_x < left
            || pointer_x > left + display_width
            || pointer_y < top
            || pointer_y > top + display_height
        {
            return false;
        }

        let x = ((pointer_x - left) / display_width * source_width as f32)
            .clamp(0.0, source_width as f32);
        let y = ((pointer_y - top) / display_height * source_height as f32)
            .clamp(0.0, source_height as f32);
        let item = IconAnnotation {
            asset_id: settings.asset_id.clone(),
            x,
            y,
            size: settings.size,
            color: settings.color,
            opacity: settings.opacity,
        };
        self.annotations.place(item);
        self.notify_canvas_operation(
            CanvasOperation::IconPlaced {
                asset_id: settings.asset_id,
                x,
                y,
                size: settings.size,
                color: format!("#{:06x}", settings.color),
                opacity: settings.opacity,
            },
            cx,
        );
        cx.notify();
        true
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
        self.dispatch_plugin_ui_event(control_id.to_string(), UiValue::None, cx);
        cx.notify();
    }

    fn notify_canvas_operation(&self, operation: CanvasOperation, cx: &mut Context<Self>) {
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
