use gpui::{
    div, img, px, rgb, App, Context, ExternalPaths, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, ObjectFit, ParentElement, Render, ScrollDelta,
    ScrollWheelEvent, Styled, StyledImage, Window,
};

use crate::app::LumiaApp;
use crate::i18n::{tr, TextKey};
use crate::palette::Palette;
use crate::util::status_message;
use crate::widgets::{context_menu_item, toolbar_button};
use crate::{Quit, TOOLBAR_HEIGHT};

impl Render for LumiaApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette(window);

        div()
            .id("lumia-root")
            .track_focus(&self.focus_handle)
            .key_context("Lumia")
            .relative()
            .on_action(cx.listener(Self::open_file))
            .on_action(cx.listener(Self::zoom_in))
            .on_action(cx.listener(Self::zoom_out))
            .on_action(cx.listener(Self::zoom_fit))
            .on_action(cx.listener(Self::toggle_fullscreen))
            .on_action(cx.listener(Self::exit_fullscreen))
            .on_action(cx.listener(Self::toggle_image_info))
            .on_action(|_: &Quit, _: &mut Window, cx: &mut App| cx.quit())
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(palette.viewer_bg))
            .text_color(rgb(palette.text))
            .children((!self.is_fullscreen).then(|| self.render_toolbar(palette, cx)))
            .child(self.render_viewer(window, palette, cx))
            .children(self.render_settings_panel(window, palette, cx))
    }
}

impl LumiaApp {
    fn render_toolbar(&self, palette: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let language = self.settings.language;

        div()
            .id("toolbar")
            .h(px(TOOLBAR_HEIGHT))
            .w_full()
            .flex()
            .items_center()
            .gap_2()
            .px_4()
            .border_b_1()
            .border_color(rgb(palette.border))
            .bg(rgb(palette.toolbar_bg))
            .child(toolbar_button(
                "open-button",
                tr(language, TextKey::Open),
                palette,
                cx,
                |this, _, window, cx| {
                    this.open_file_dialog(cx, Some(window));
                },
            ))
            .child(toolbar_button(
                "fit-button",
                tr(language, TextKey::Fit),
                palette,
                cx,
                |this, _, _, cx| {
                    this.reset_fit(cx);
                },
            ))
            .child(toolbar_button(
                "fullscreen-button",
                tr(language, TextKey::Full),
                palette,
                cx,
                |this, _, window, cx| {
                    this.toggle_window_fullscreen(window, cx);
                },
            ))
            .child(
                div()
                    .px_2()
                    .text_sm()
                    .text_color(rgb(palette.muted_text))
                    .child(format!("{:.0}%", self.viewport.zoom * 100.0)),
            )
            .child(div().flex_1())
            .child(toolbar_button(
                "settings-button",
                tr(language, TextKey::Settings),
                palette,
                cx,
                |this, _, _, cx| {
                    this.open_settings_panel(cx);
                },
            ))
    }

    fn render_viewer(
        &self,
        window: &Window,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let viewer = div()
            .id("viewer")
            .flex_1()
            .overflow_hidden()
            .flex()
            .items_center()
            .justify_center()
            .relative()
            .bg(rgb(palette.viewer_bg))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                this.pending_drop_paths = paths.paths().to_vec();
                this.load_first_supported_drop(window);
                cx.notify();
            }))
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                let delta = match event.delta {
                    ScrollDelta::Pixels(delta) => f32::from(delta.y),
                    ScrollDelta::Lines(delta) => delta.y,
                };
                if delta > 0.0 {
                    this.viewport.zoom_out();
                } else if delta < 0.0 {
                    this.viewport.zoom_in();
                }
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    if this.context_menu_position.take().is_some() {
                        cx.notify();
                        return;
                    }
                    if this.current_image.is_some() {
                        this.is_panning = true;
                        this.last_mouse_position = Some(event.position);
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.is_panning = false;
                    this.last_mouse_position = None;
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    let chrome_height = if this.is_fullscreen {
                        0.0
                    } else {
                        TOOLBAR_HEIGHT
                    };
                    this.context_menu_position = Some(gpui::point(
                        event.position.x,
                        px((f32::from(event.position.y) - chrome_height).max(0.0)),
                    ));
                    this.is_panning = false;
                    this.last_mouse_position = None;
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.is_panning = false;
                    this.last_mouse_position = None;
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if this.is_panning && event.dragging() {
                    if let Some(last_position) = this.last_mouse_position {
                        this.viewport.pan_by(
                            f32::from(event.position.x - last_position.x),
                            f32::from(event.position.y - last_position.y),
                        );
                    }
                    this.last_mouse_position = Some(event.position);
                    cx.notify();
                }
            }));

        if let Some(message) = &self.error_message {
            viewer
                .child(status_message("error-state", message, palette.error_text))
                .children(self.render_image_info_overlay())
                .children(self.render_context_menu(palette, cx))
        } else if let Some(path) = self.image_path() {
            let image = if let Some((width, height)) = self.scaled_image_size(window) {
                img(path.to_path_buf())
                    .w(px(width))
                    .h(px(height))
                    .object_fit(ObjectFit::Contain)
                    .into_any_element()
            } else {
                img(path.to_path_buf())
                    .max_w_full()
                    .max_h_full()
                    .object_fit(ObjectFit::Contain)
                    .into_any_element()
            };

            viewer
                .child(
                    div()
                        .ml(px(self.viewport.pan_x))
                        .mt(px(self.viewport.pan_y))
                        .child(image),
                )
                .children(self.render_image_info_overlay())
                .children(self.render_context_menu(palette, cx))
        } else {
            viewer
                .child(status_message(
                    "empty-state",
                    tr(self.settings.language, TextKey::EmptyState),
                    palette.muted_text,
                ))
                .children(self.render_image_info_overlay())
                .children(self.render_context_menu(palette, cx))
        }
    }

    fn render_context_menu(
        &self,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let language = self.settings.language;

        self.context_menu_position.map(|position| {
            div()
                .id("viewer-context-menu")
                .absolute()
                .left(position.x)
                .top(position.y)
                .w(px(156.0))
                .py_1()
                .rounded_md()
                .bg(rgb(palette.panel_bg))
                .border_1()
                .border_color(rgb(palette.border))
                .shadow_lg()
                .text_color(rgb(palette.text))
                .text_sm()
                .child(context_menu_item(
                    "settings-menu-item",
                    tr(language, TextKey::Settings),
                    palette,
                    cx,
                    |this, _, _, cx| {
                        this.open_settings_panel(cx);
                    },
                ))
                .child(context_menu_item(
                    "about-menu-item",
                    tr(language, TextKey::About),
                    palette,
                    cx,
                    |this, _, _, cx| {
                        this.context_menu_position = None;
                        cx.notify();
                    },
                ))
                .child(context_menu_item(
                    "quit-menu-item",
                    tr(language, TextKey::Quit),
                    palette,
                    cx,
                    |this, _, _, cx| {
                        this.context_menu_position = None;
                        cx.quit();
                    },
                ))
        })
    }
}
