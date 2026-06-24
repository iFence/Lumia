use gpui::{
    div, img, px, rgb, App, Context, ExternalPaths, FontWeight, InteractiveElement, IntoElement,
    MouseButton, MouseDownEvent, MouseMoveEvent, ObjectFit, ParentElement, Render, ScrollDelta,
    ScrollWheelEvent, StatefulInteractiveElement, Styled, StyledImage, Window,
};

use crate::app::LumiaApp;
use crate::i18n::{tr, TextKey};
use crate::palette::Palette;
use crate::util::status_message;
use crate::widgets::{context_menu_item, titlebar_button, titlebar_close_button, toolbar_button};
use crate::{Quit, TOOLBAR_HEIGHT, TITLE_BAR_HEIGHT};

impl Render for LumiaApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Non-blocking poll: if a background HEIC→PNG decode completed, apply
        // the result now so this frame renders with the decoded image.
        self.poll_decode(cx);

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
            .on_action(cx.listener(Self::next_image))
            .on_action(cx.listener(Self::previous_image))
            .on_action(|_: &Quit, _: &mut Window, cx: &mut App| cx.quit())
            .on_mouse_move(cx.listener(Self::handle_root_mouse_move))
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(palette.viewer_bg))
            .text_color(rgb(palette.text))
            .children(
                self.should_show_titlebar()
                    .then(|| self.render_titlebar(palette, cx)),
            )
            .children(
                self.should_show_toolbar()
                    .then(|| self.render_toolbar(palette, cx)),
            )
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
            .children(self.render_position_indicator(palette))
            .child(div().flex_1())
            .child(toolbar_button(
                "lock-button",
                if self.toolbar_locked {
                    tr(language, TextKey::Unlock)
                } else {
                    tr(language, TextKey::Lock)
                },
                palette,
                cx,
                |this, _, _, cx| {
                    this.toggle_toolbar_lock(cx);
                },
            ))
    }

    fn render_titlebar(&self, palette: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("titlebar")
            .h(px(TITLE_BAR_HEIGHT))
            .w_full()
            .flex()
            .items_center()
            .border_b_1()
            .border_color(rgb(palette.border))
            .bg(rgb(palette.selection_bg))
            .child(
                div()
                    .id("titlebar-drag-region")
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .pl_3()
                    .on_mouse_down(MouseButton::Left, cx.listener(|_, _, window, _| {
                        window.start_window_move();
                    }))
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(palette.muted_text))
                            .child(self.window_title.clone()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(titlebar_button(
                        "titlebar-minimize",
                        "\u{2014}",
                        palette,
                        cx,
                        |_, _, window, _| {
                            window.minimize_window();
                        },
                    ))
                    .child(titlebar_button(
                        "titlebar-maximize",
                        if self.is_fullscreen { "\u{29C9}" } else { "\u{25A1}" },
                        palette,
                        cx,
                        |this, _, window, cx| {
                            this.toggle_window_fullscreen(window, cx);
                        },
                    ))
                    .child(titlebar_close_button(
                        "titlebar-close",
                        "\u{2715}",
                        palette,
                        cx,
                        |_, _, _, cx| {
                            cx.quit();
                        },
                    )),
            )
    }

    fn render_position_indicator(&self, palette: Palette) -> Option<impl IntoElement> {
        let count = self.sibling_count();
        if count <= 1 {
            return None;
        }
        let current = self.current_image_index().map(|i| i + 1).unwrap_or(0);
        Some(
            div()
                .px_2()
                .text_sm()
                .text_color(rgb(palette.muted_text))
                .child(format!("{current} / {count}")),
        )
    }

    fn render_empty_state(
        &self,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let language = self.settings.language;

        div()
            .id("empty-state")
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .child(
                div()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.text))
                    .child("Lumia"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(palette.muted_text))
                    .child(tr(language, TextKey::EmptyState)),
            )
            .child(
                div()
                    .id("empty-state-open-button")
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(palette.accent))
                    .text_color(rgb(0xffffff))
                    .text_sm()
                    .font_weight(FontWeight::MEDIUM)
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(palette.accent_bg)))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_file_dialog(cx, Some(window));
                    }))
                    .child(tr(language, TextKey::EmptyStateOpenButton)),
            )
    }

    fn poll_decode(&mut self, cx: &mut Context<Self>) {
        if let Some(ref rx) = self.pending_decode {
            match rx.try_recv() {
                Ok(cached) => {
                    if let Some(ref mut doc) = self.current_image {
                        doc.cached_image = cached;
                    }
                    self.is_decoding = false;
                    self.pending_decode = None;
                    cx.notify();
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.is_decoding = false;
                    self.pending_decode = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    // Still decoding — check again next frame.
                }
            }
        }
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
                if this.show_settings_panel {
                    return;
                }
                this.pending_drop_paths = paths.paths().to_vec();
                this.load_first_supported_drop(window, cx);
            }))
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                if this.show_settings_panel {
                    return;
                }
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
                    if this.show_settings_panel {
                        return;
                    }
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
                    if this.show_settings_panel {
                        return;
                    }
                    this.is_panning = false;
                    this.last_mouse_position = None;
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    if this.show_settings_panel {
                        return;
                    }
                    let chrome_height = if this.is_fullscreen {
                        if this.should_show_toolbar() {
                            TITLE_BAR_HEIGHT + TOOLBAR_HEIGHT
                        } else {
                            0.0
                        }
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
                    if this.show_settings_panel {
                        return;
                    }
                    this.is_panning = false;
                    this.last_mouse_position = None;
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if this.show_settings_panel {
                    return;
                }
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
            // Formats like HEIC are pre-decoded and cached as PNG at load
            // time (see ImageDocument::load_from_path) because GPUI's img()
            // cannot natively render them.
            let cached = self
                .current_image
                .as_ref()
                .and_then(|doc| doc.cached_image.as_ref());

            let image = if let Some(cached) = cached {
                let (display_w, display_h) = self
                    .scaled_image_size(window)
                    .unwrap_or((cached.width as f32, cached.height as f32));
                let gpui_image = gpui::Image::from_bytes(
                    gpui::ImageFormat::Png,
                    cached.png_data.clone(),
                );
                img(std::sync::Arc::new(gpui_image))
                    .w(px(display_w))
                    .h(px(display_h))
                    .object_fit(ObjectFit::Contain)
                    .into_any_element()
            } else if let Some((width, height)) = self.scaled_image_size(window) {
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
                .children(self.render_decoding_overlay(palette))
                .children(self.render_image_info_overlay())
                .children(self.render_context_menu(palette, cx))
        } else {
            viewer
                .child(self.render_empty_state(palette, cx))
                .children(self.render_image_info_overlay())
                .children(self.render_context_menu(palette, cx))
        }
    }

    fn render_decoding_overlay(&self, palette: Palette) -> Option<impl IntoElement> {
        if !self.is_decoding {
            return None;
        }
        Some(
            div()
                .absolute()
                .bottom_4()
                .right_4()
                .px_3()
                .py_1()
                .rounded_md()
                .bg(rgb(palette.button_bg))
                .text_color(rgb(palette.muted_text))
                .text_sm()
                .child("Decoding…"),
        )
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
