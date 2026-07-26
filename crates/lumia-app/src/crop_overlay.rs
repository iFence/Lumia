use gpui::{
    div, px, rgb, AnyElement, Context, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, ParentElement, Styled,
};
use lumia_core::CropRect;

use crate::app::LumiaApp;
use crate::editing::{CropDrag, CropDragKind, EditMode};
use crate::palette::Palette;

const HANDLE_SIZE: f32 = 12.0;

impl LumiaApp {
    pub(crate) fn render_crop_overlay(
        &self,
        scale: f32,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if self.editing.mode != Some(EditMode::Crop) || scale <= 0.0 {
            return None;
        }
        let rect = self.editing.crop_rect;
        let left = rect.x as f32 * scale;
        let top = rect.y as f32 * scale;
        let width = rect.width as f32 * scale;
        let height = rect.height as f32 * scale;
        let image_width = self.editing.source_width as f32 * scale;
        let image_height = self.editing.source_height as f32 * scale;
        let mask = gpui::black().opacity(0.5);

        Some(
            div()
                .id("crop-overlay")
                .absolute()
                .inset_0()
                .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                    if event.dragging() {
                        this.update_crop_drag(event, scale, cx);
                    }
                }))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.editing.crop_drag = None;
                        cx.notify();
                    }),
                )
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.editing.crop_drag = None;
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .w(px(image_width))
                        .h(px(top.max(0.0)))
                        .bg(mask),
                )
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top(px(top))
                        .w(px(left.max(0.0)))
                        .h(px(height))
                        .bg(mask),
                )
                .child(
                    div()
                        .absolute()
                        .left(px(left + width))
                        .top(px(top))
                        .w(px((image_width - left - width).max(0.0)))
                        .h(px(height))
                        .bg(mask),
                )
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .top(px(top + height))
                        .w(px(image_width))
                        .h(px((image_height - top - height).max(0.0)))
                        .bg(mask),
                )
                .child(
                    div()
                        .id("crop-selection")
                        .absolute()
                        .left(px(left))
                        .top(px(top))
                        .w(px(width))
                        .h(px(height))
                        .border_2()
                        .border_color(rgb(palette.accent))
                        .cursor_move()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, event: &MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                this.begin_crop_drag(CropDragKind::Move, event, cx);
                            }),
                        )
                        .children(
                            [
                                CropDragKind::TopLeft,
                                CropDragKind::TopRight,
                                CropDragKind::BottomLeft,
                                CropDragKind::BottomRight,
                            ]
                            .into_iter()
                            .map(|kind| self.render_crop_handle(kind, palette, cx)),
                        ),
                )
                .into_any_element(),
        )
    }

    fn render_crop_handle(
        &self,
        kind: CropDragKind,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let id = match kind {
            CropDragKind::TopLeft => "crop-handle-top-left",
            CropDragKind::TopRight => "crop-handle-top-right",
            CropDragKind::BottomLeft => "crop-handle-bottom-left",
            CropDragKind::BottomRight => "crop-handle-bottom-right",
            CropDragKind::Move => unreachable!(),
        };
        let mut handle = div()
            .id(id)
            .absolute()
            .w(px(HANDLE_SIZE))
            .h(px(HANDLE_SIZE))
            .rounded_sm()
            .border_1()
            .border_color(gpui::white())
            .bg(rgb(palette.accent))
            .cursor_crosshair()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.begin_crop_drag(kind, event, cx);
                }),
            );
        handle = match kind {
            CropDragKind::TopLeft => handle
                .left(px(-HANDLE_SIZE / 2.0))
                .top(px(-HANDLE_SIZE / 2.0)),
            CropDragKind::TopRight => handle
                .right(px(-HANDLE_SIZE / 2.0))
                .top(px(-HANDLE_SIZE / 2.0)),
            CropDragKind::BottomLeft => handle
                .left(px(-HANDLE_SIZE / 2.0))
                .bottom(px(-HANDLE_SIZE / 2.0)),
            CropDragKind::BottomRight => handle
                .right(px(-HANDLE_SIZE / 2.0))
                .bottom(px(-HANDLE_SIZE / 2.0)),
            CropDragKind::Move => unreachable!(),
        };
        handle.into_any_element()
    }

    fn begin_crop_drag(
        &mut self,
        kind: CropDragKind,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        self.editing.crop_drag = Some(CropDrag {
            kind,
            start_x: f32::from(event.position.x),
            start_y: f32::from(event.position.y),
            start_rect: self.editing.crop_rect,
        });
        self.editing.feedback = None;
        cx.notify();
    }

    fn update_crop_drag(&mut self, event: &MouseMoveEvent, scale: f32, cx: &mut Context<Self>) {
        let Some(drag) = self.editing.crop_drag else {
            return;
        };
        let dx = (f32::from(event.position.x) - drag.start_x) / scale;
        let dy = (f32::from(event.position.y) - drag.start_y) / scale;
        self.editing.crop_rect = updated_crop_rect(
            drag,
            dx,
            dy,
            self.editing.source_width,
            self.editing.source_height,
            self.editing
                .crop_aspect
                .ratio(self.editing.source_width, self.editing.source_height),
        );
        cx.notify();
    }
}

fn updated_crop_rect(
    drag: CropDrag,
    dx: f32,
    dy: f32,
    image_width: u32,
    image_height: u32,
    ratio: Option<f32>,
) -> CropRect {
    let start = drag.start_rect;
    if drag.kind == CropDragKind::Move {
        return CropRect::new(
            (start.x as f32 + dx)
                .round()
                .clamp(0.0, image_width.saturating_sub(start.width) as f32) as u32,
            (start.y as f32 + dy)
                .round()
                .clamp(0.0, image_height.saturating_sub(start.height) as f32) as u32,
            start.width,
            start.height,
        );
    }

    let left = start.x as f32;
    let top = start.y as f32;
    let right = left + start.width as f32;
    let bottom = top + start.height as f32;
    let (anchor_x, anchor_y, moving_x, moving_y) = match drag.kind {
        CropDragKind::TopLeft => (right, bottom, left + dx, top + dy),
        CropDragKind::TopRight => (left, bottom, right + dx, top + dy),
        CropDragKind::BottomLeft => (right, top, left + dx, bottom + dy),
        CropDragKind::BottomRight => (left, top, right + dx, bottom + dy),
        CropDragKind::Move => unreachable!(),
    };
    let mut width = (moving_x - anchor_x).abs().max(1.0);
    let mut height = (moving_y - anchor_y).abs().max(1.0);
    if let Some(ratio) = ratio {
        if width / height > ratio {
            width = height * ratio;
        } else {
            height = width / ratio;
        }
    }
    let moving_left = moving_x < anchor_x;
    let moving_top = moving_y < anchor_y;
    let max_width = if moving_left {
        anchor_x
    } else {
        image_width as f32 - anchor_x
    }
    .max(1.0);
    let max_height = if moving_top {
        anchor_y
    } else {
        image_height as f32 - anchor_y
    }
    .max(1.0);
    let scale = (max_width / width).min(max_height / height).min(1.0);
    width = (width * scale).max(1.0);
    height = (height * scale).max(1.0);
    let x = if moving_left {
        anchor_x - width
    } else {
        anchor_x
    };
    let y = if moving_top {
        anchor_y - height
    } else {
        anchor_y
    };
    let x = x.round().clamp(0.0, image_width.saturating_sub(1) as f32) as u32;
    let y = y.round().clamp(0.0, image_height.saturating_sub(1) as f32) as u32;
    CropRect::new(
        x,
        y,
        (width.round().max(1.0) as u32).min(image_width - x),
        (height.round().max(1.0) as u32).min(image_height - y),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moving_a_crop_is_clamped_to_the_image() {
        let rect = updated_crop_rect(
            CropDrag {
                kind: CropDragKind::Move,
                start_x: 0.0,
                start_y: 0.0,
                start_rect: CropRect::new(10, 10, 40, 30),
            },
            100.0,
            -100.0,
            80,
            60,
            None,
        );
        assert_eq!(rect, CropRect::new(40, 0, 40, 30));
    }

    #[test]
    fn locked_corner_drag_preserves_ratio_and_bounds() {
        let rect = updated_crop_rect(
            CropDrag {
                kind: CropDragKind::BottomRight,
                start_x: 0.0,
                start_y: 0.0,
                start_rect: CropRect::new(0, 0, 40, 20),
            },
            100.0,
            100.0,
            100,
            100,
            Some(2.0),
        );
        assert_eq!((rect.width, rect.height), (100, 50));
    }
}
