use gpui::{
    div, img, px, rgb, AnyElement, Context, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, ObjectFit, ParentElement, Point, Styled, StyledImage, Window,
};

use crate::app::LumiaApp;
use crate::palette::Palette;
use crate::STATUS_BAR_HEIGHT;

const MAX_THUMBNAIL_WIDTH: f32 = 220.0;
const MAX_THUMBNAIL_HEIGHT: f32 = 150.0;
const PANEL_PADDING: f32 = 8.0;
const PANEL_MARGIN: f32 = 12.0;
const OVERFLOW_TOLERANCE: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq)]
struct OverviewGeometry {
    thumbnail_left: f32,
    thumbnail_top: f32,
    thumbnail_width: f32,
    thumbnail_height: f32,
    viewport_left: f32,
    viewport_top: f32,
    viewport_width: f32,
    viewport_height: f32,
    display_width: f32,
    display_height: f32,
    window_width: f32,
    window_height: f32,
}

impl OverviewGeometry {
    fn calculate(
        display_width: f32,
        display_height: f32,
        window_width: f32,
        window_height: f32,
        pan_x: f32,
        pan_y: f32,
        bottom_offset: f32,
    ) -> Option<Self> {
        let values = [
            display_width,
            display_height,
            window_width,
            window_height,
            pan_x,
            pan_y,
            bottom_offset,
        ];
        if values.iter().any(|value| !value.is_finite())
            || display_width <= 0.0
            || display_height <= 0.0
            || window_width <= 0.0
            || window_height <= 0.0
            || (display_width <= window_width + OVERFLOW_TOLERANCE
                && display_height <= window_height + OVERFLOW_TOLERANCE)
        {
            return None;
        }

        let thumbnail_scale =
            (MAX_THUMBNAIL_WIDTH / display_width).min(MAX_THUMBNAIL_HEIGHT / display_height);
        let thumbnail_width = display_width * thumbnail_scale;
        let thumbnail_height = display_height * thumbnail_scale;
        let thumbnail_left = window_width - PANEL_MARGIN - PANEL_PADDING - thumbnail_width;
        let thumbnail_top = window_height - bottom_offset - PANEL_PADDING - thumbnail_height;

        let max_pan_x = ((display_width - window_width) / 2.0).max(0.0);
        let max_pan_y = ((display_height - window_height) / 2.0).max(0.0);
        let visible_left = if max_pan_x > 0.0 {
            (display_width - window_width) / 2.0 - pan_x.clamp(-max_pan_x, max_pan_x)
        } else {
            0.0
        };
        let visible_top = if max_pan_y > 0.0 {
            (display_height - window_height) / 2.0 - pan_y.clamp(-max_pan_y, max_pan_y)
        } else {
            0.0
        };

        Some(Self {
            thumbnail_left,
            thumbnail_top,
            thumbnail_width,
            thumbnail_height,
            viewport_left: visible_left * thumbnail_scale,
            viewport_top: visible_top * thumbnail_scale,
            viewport_width: window_width.min(display_width) * thumbnail_scale,
            viewport_height: window_height.min(display_height) * thumbnail_scale,
            display_width,
            display_height,
            window_width,
            window_height,
        })
    }

    fn pan_for_position(&self, x: f32, y: f32) -> (f32, f32) {
        let normalized_x = ((x - self.thumbnail_left) / self.thumbnail_width).clamp(0.0, 1.0);
        let normalized_y = ((y - self.thumbnail_top) / self.thumbnail_height).clamp(0.0, 1.0);
        let max_pan_x = ((self.display_width - self.window_width) / 2.0).max(0.0);
        let max_pan_y = ((self.display_height - self.window_height) / 2.0).max(0.0);

        (
            ((0.5 - normalized_x) * self.display_width).clamp(-max_pan_x, max_pan_x),
            ((0.5 - normalized_y) * self.display_height).clamp(-max_pan_y, max_pan_y),
        )
    }
}

impl LumiaApp {
    pub(crate) fn render_image_overview(
        &self,
        window: &Window,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let geometry = self.overview_geometry(window)?;
        let path = self.image_path()?;
        let thumbnail = if let Some(prepared) = self
            .loads
            .display_image(self.viewer.rotation_quarter_turns())
        {
            img(prepared.render_image())
                .size_full()
                .object_fit(ObjectFit::Contain)
                .into_any_element()
        } else if self.loads.is_decoding() {
            return None;
        } else {
            img(path.to_path_buf())
                .size_full()
                .object_fit(ObjectFit::Contain)
                .into_any_element()
        };
        let bottom = self.overview_bottom_offset();

        Some(
            div()
                .id("image-overview")
                .absolute()
                .right(px(PANEL_MARGIN))
                .bottom(px(bottom))
                .p(px(PANEL_PADDING))
                .rounded_md()
                .border_1()
                .border_color(gpui::white().opacity(0.22))
                .bg(gpui::black().opacity(0.74))
                .shadow_md()
                .cursor_pointer()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, event: &MouseDownEvent, window, cx| {
                        cx.stop_propagation();
                        this.ui.is_overview_panning = true;
                        this.pan_from_overview(event.position, window);
                        this.refresh_large_image_tiles(window, cx);
                        cx.notify();
                    }),
                )
                .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, window, cx| {
                    cx.stop_propagation();
                    if this.ui.is_overview_panning && event.dragging() {
                        this.pan_from_overview(event.position, window);
                        this.refresh_large_image_tiles(window, cx);
                        cx.notify();
                    }
                }))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.ui.is_overview_panning = false;
                        cx.notify();
                    }),
                )
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.ui.is_overview_panning = false;
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .relative()
                        .overflow_hidden()
                        .w(px(geometry.thumbnail_width))
                        .h(px(geometry.thumbnail_height))
                        .bg(rgb(palette.viewer_bg))
                        .child(thumbnail)
                        .child(
                            div()
                                .absolute()
                                .left(px(geometry.viewport_left))
                                .top(px(geometry.viewport_top))
                                .w(px(geometry.viewport_width))
                                .h(px(geometry.viewport_height))
                                .rounded_sm()
                                .border_2()
                                .border_color(gpui::white().opacity(0.95)),
                        ),
                )
                .into_any_element(),
        )
    }

    fn overview_geometry(&self, window: &Window) -> Option<OverviewGeometry> {
        let (display_width, display_height) = self.scaled_image_size(window)?;
        let window_size = window.viewport_size();
        OverviewGeometry::calculate(
            display_width,
            display_height,
            f32::from(window_size.width),
            f32::from(window_size.height),
            self.viewer.viewport().pan_x,
            self.viewer.viewport().pan_y,
            self.overview_bottom_offset(),
        )
    }

    fn overview_bottom_offset(&self) -> f32 {
        if self.ui.show_status_bar || self.ui.show_zoom_menu {
            STATUS_BAR_HEIGHT + PANEL_MARGIN
        } else {
            PANEL_MARGIN
        }
    }

    fn pan_from_overview(&mut self, position: Point<gpui::Pixels>, window: &Window) {
        let Some(geometry) = self.overview_geometry(window) else {
            return;
        };
        let (pan_x, pan_y) =
            geometry.pan_for_position(f32::from(position.x), f32::from(position.y));
        self.viewer.viewport_mut().set_pan(pan_x, pan_y);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overview_only_appears_when_scaled_image_overflows() {
        assert!(OverviewGeometry::calculate(800.0, 600.0, 1000.0, 800.0, 0.0, 0.0, 12.0).is_none());
        assert!(
            OverviewGeometry::calculate(1000.25, 800.0, 1000.0, 800.0, 0.0, 0.0, 12.0,).is_none()
        );
        assert!(
            OverviewGeometry::calculate(1200.0, 600.0, 1000.0, 800.0, 0.0, 0.0, 12.0).is_some()
        );
    }

    #[test]
    fn viewport_frame_and_navigation_follow_pan() {
        let centered =
            OverviewGeometry::calculate(2000.0, 1000.0, 1000.0, 800.0, 0.0, 0.0, 12.0).unwrap();
        assert_eq!(centered.thumbnail_width, 220.0);
        assert_eq!(centered.thumbnail_height, 110.0);
        assert_eq!(centered.viewport_left, 55.0);
        assert_eq!(centered.viewport_width, 110.0);
        assert_eq!(centered.viewport_top, 11.0);
        assert_eq!(centered.viewport_height, 88.0);

        let right_edge = centered.thumbnail_left + centered.thumbnail_width;
        let middle_y = centered.thumbnail_top + centered.thumbnail_height / 2.0;
        assert_eq!(
            centered.pan_for_position(right_edge, middle_y),
            (-500.0, 0.0)
        );
    }
}
