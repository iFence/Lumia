use gpui::{
    div, img, px, rgb, AnyElement, App, Context, ExternalPaths, FontWeight, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent, ObjectFit, ParentElement, Render,
    ScrollDelta, ScrollWheelEvent, StatefulInteractiveElement, Styled, StyledImage, Window,
};
use gpui_component::{Icon, IconName, Theme, ThemeMode as ComponentThemeMode};
use lumia_core::FitMode;

use crate::app::LumiaApp;
use crate::i18n::{tr, TextKey};
use crate::palette::{theme_resolves_to_dark, Palette};
use crate::util::{format_file_size, status_message};
use crate::widgets::context_menu_item;
use crate::{
    Quit, STATUS_BAR_HEIGHT, ZOOM_MENU_BOTTOM_GAP, ZOOM_MENU_ITEM_HEIGHT, ZOOM_MENU_RIGHT,
    ZOOM_MENU_WIDTH,
};

impl Render for LumiaApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Non-blocking poll: drain completed adjacent-image preloads.
        self.poll_preloads(cx);

        let component_theme_mode =
            if theme_resolves_to_dark(self.settings.theme, window.appearance()) {
                ComponentThemeMode::Dark
            } else {
                ComponentThemeMode::Light
            };
        if Theme::global(cx).mode != component_theme_mode {
            Theme::change(component_theme_mode, None, cx);
        }

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
            .on_action(cx.listener(Self::rotate_clockwise))
            .on_action(cx.listener(Self::rotate_counter_clockwise))
            .on_action(|_: &Quit, _: &mut Window, cx: &mut App| cx.quit())
            .on_mouse_move(cx.listener(Self::handle_root_mouse_move))
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(palette.viewer_bg))
            .text_color(rgb(palette.text))
            .child(self.render_viewer(window, palette, cx))
            .children(
                (self.show_status_bar || self.show_zoom_menu)
                    .then(|| self.render_status_bar(palette, cx)),
            )
            .children(self.render_settings_panel(window, palette, cx))
    }
}

impl LumiaApp {
    fn render_empty_state(&self, palette: Palette, _cx: &mut Context<Self>) -> impl IntoElement {
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
                    .cursor_pointer()
                    .bg(rgb(palette.accent))
                    .text_color(rgb(palette.accent_text))
                    .hover(move |style| style.bg(rgb(palette.accent_hover)))
                    .active(move |style| style.bg(rgb(palette.accent_active)))
                    .on_mouse_down(MouseButton::Left, {
                        let self_handle = self.self_handle.clone();
                        move |_, window, cx| {
                            let _ = self_handle.update(cx, |this, cx| {
                                this.open_file_dialog(cx, Some(window));
                            });
                        }
                    })
                    .child(tr(language, TextKey::EmptyStateOpenButton)),
            )
    }

    fn poll_preloads(&mut self, _cx: &mut Context<Self>) {
        // Drain completed preload receivers and stash results in the cache.
        self.pending_preloads.retain(|rx| {
            match rx.try_recv() {
                Ok(Some((path, cached))) => {
                    self.preload_cache.insert(path, cached);
                    false // remove this receiver
                }
                Ok(None) => {
                    // Decode failed — nothing to cache.
                    false
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => false,
                Err(std::sync::mpsc::TryRecvError::Empty) => true, // still running
            }
        });
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
                    this.context_menu_position =
                        Some(gpui::point(event.position.x, event.position.y));
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
            // Check the document's own cache first, then the preload cache
            // (populated by background adjacent-image decoding).
            let cached = self.rotated_image.as_ref().or_else(|| {
                (self.rotation_quarter_turns == 0)
                    .then(|| {
                        self.current_image
                            .as_ref()
                            .and_then(|doc| doc.cached_image.as_ref())
                            .or_else(|| self.image_path().and_then(|p| self.preload_cache.get(p)))
                    })
                    .flatten()
            });

            let image = if let Some(cached) = cached {
                let (display_w, display_h) = self
                    .scaled_image_size(window)
                    .unwrap_or((cached.width as f32, cached.height as f32));
                let gpui_image =
                    gpui::Image::from_bytes(gpui::ImageFormat::Bmp, cached.cached_data.clone());
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

    fn render_status_bar(&self, palette: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let has_image = self.current_image.is_some();
        let count = self.sibling_count();
        let current = self
            .current_image_index()
            .map(|index| index + 1)
            .unwrap_or(0);
        let dimensions = self
            .current_image
            .as_ref()
            .and_then(|image| image.metadata.as_ref())
            .map(|metadata| format!("{}x{}", metadata.width, metadata.height))
            .unwrap_or_else(|| "--".to_string());
        let file_size = self
            .image_path()
            .and_then(|path| std::fs::metadata(path).ok())
            .map(|metadata| format_file_size(metadata.len()))
            .unwrap_or_else(|| "--".to_string());
        div()
            .id("status-bar")
            .absolute()
            .left_0()
            .right_0()
            .bottom_0()
            .h(px(STATUS_BAR_HEIGHT))
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .px_4()
            .border_t_1()
            .border_color(rgb(palette.border))
            .bg(rgb(palette.toolbar_bg))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(self.render_status_icon_button(
                        "status-prev-image",
                        IconName::ChevronLeft,
                        has_image && current > 1,
                        palette,
                        cx,
                        |this, _, window, cx| {
                            this.navigate_image(-1, window, cx);
                            cx.notify();
                        },
                    ))
                    .child(self.render_status_text(format!("{current}/{count}"), palette))
                    .child(self.render_status_icon_button(
                        "status-next-image",
                        IconName::ChevronRight,
                        has_image && current < count,
                        palette,
                        cx,
                        |this, _, window, cx| {
                            this.navigate_image(1, window, cx);
                            cx.notify();
                        },
                    ))
                    .child(self.render_status_icon_button(
                        "status-rotate-counter-clockwise",
                        IconName::Undo2,
                        has_image,
                        palette,
                        cx,
                        |this, _, _, cx| {
                            this.rotate_display(3, cx);
                        },
                    ))
                    .child(self.render_status_icon_button(
                        "status-rotate-clockwise",
                        IconName::Redo2,
                        has_image,
                        palette,
                        cx,
                        |this, _, _, cx| {
                            this.rotate_display(1, cx);
                        },
                    ))
                    .child(self.render_status_text(file_size, palette))
                    .child(self.render_status_text(dimensions, palette)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(self.render_status_icon_button(
                        "status-fit-toggle",
                        if self.viewport.fit_mode == FitMode::FitToWindow {
                            IconName::Minimize
                        } else {
                            IconName::Maximize
                        },
                        has_image,
                        palette,
                        cx,
                        |this, _, _, cx| {
                            this.toggle_fit_or_actual_size(cx);
                        },
                    ))
                    .child(self.render_status_zoom_button(
                        has_image,
                        palette,
                        cx,
                        |this, _, _, cx| {
                            this.toggle_zoom_menu(cx);
                        },
                    ))
                    .child(self.render_status_icon_button(
                        "status-zoom-in",
                        IconName::Plus,
                        has_image,
                        palette,
                        cx,
                        |this, _, _, cx| {
                            if this.current_image.is_some() {
                                this.viewport.zoom_in();
                                this.show_zoom_menu = false;
                                cx.notify();
                            }
                        },
                    ))
                    .child(self.render_status_icon_button(
                        "status-zoom-out",
                        IconName::Minus,
                        has_image,
                        palette,
                        cx,
                        |this, _, _, cx| {
                            if this.current_image.is_some() {
                                this.viewport.zoom_out();
                                this.show_zoom_menu = false;
                                cx.notify();
                            }
                        },
                    ))
                    .child(self.render_status_icon_button(
                        "status-fullscreen",
                        IconName::Maximize,
                        true,
                        palette,
                        cx,
                        |this, _, window, cx| {
                            this.toggle_window_fullscreen(window, cx);
                        },
                    )),
            )
            .children(self.render_zoom_menu(palette, cx))
    }

    fn render_status_zoom_button(
        &self,
        enabled: bool,
        palette: Palette,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut LumiaApp, &MouseDownEvent, &mut Window, &mut Context<LumiaApp>) + 'static,
    ) -> AnyElement {
        let text_color = if enabled {
            palette.text
        } else {
            palette.muted_text
        };

        div()
            .id("status-zoom-menu-button")
            .h(px(28.0))
            .px_2()
            .flex()
            .items_center()
            .justify_center()
            .gap_1()
            .rounded_sm()
            .border_1()
            .border_color(rgb(palette.border))
            .bg(rgb(palette.button_bg))
            .text_sm()
            .text_color(rgb(text_color))
            .hover(move |style| style.bg(rgb(palette.button_hover)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event, window, cx| {
                    if enabled {
                        on_click(this, event, window, cx);
                    }
                }),
            )
            .child(format!("{:.0}%", self.viewport.zoom * 100.0))
            .child(
                Icon::new(if self.show_zoom_menu {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronUp
                })
                .size(px(14.0))
                .text_color(rgb(text_color)),
            )
            .into_any_element()
    }
    fn render_status_icon_button(
        &self,
        id: &'static str,
        icon: IconName,
        enabled: bool,
        palette: Palette,
        cx: &mut Context<Self>,
        on_click: impl Fn(&mut LumiaApp, &MouseDownEvent, &mut Window, &mut Context<LumiaApp>) + 'static,
    ) -> AnyElement {
        let icon_color = if enabled {
            palette.text
        } else {
            palette.muted_text
        };

        div()
            .id(id)
            .w(px(32.0))
            .h(px(28.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_sm()
            .border_1()
            .border_color(rgb(palette.border))
            .bg(rgb(palette.button_bg))
            .hover(move |style| style.bg(rgb(palette.button_hover)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event, window, cx| {
                    if enabled {
                        on_click(this, event, window, cx);
                    }
                }),
            )
            .child(Icon::new(icon).size(px(16.0)).text_color(rgb(icon_color)))
            .into_any_element()
    }
    fn render_status_text(&self, label: impl Into<String>, palette: Palette) -> AnyElement {
        div()
            .px_2()
            .text_sm()
            .text_color(rgb(palette.muted_text))
            .child(label.into())
            .into_any_element()
    }

    fn render_zoom_menu(&self, palette: Palette, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.show_zoom_menu {
            return None;
        }

        let presets = [32.0, 16.0, 8.0, 4.0, 2.0, 1.5, 1.0, 0.5, 0.1];
        Some(
            div()
                .id("status-zoom-menu")
                .absolute()
                .right(px(ZOOM_MENU_RIGHT))
                .bottom(px(STATUS_BAR_HEIGHT + ZOOM_MENU_BOTTOM_GAP))
                .w(px(ZOOM_MENU_WIDTH))
                .py_2()
                .rounded_md()
                .border_1()
                .border_color(rgb(palette.border))
                .bg(rgb(palette.panel_bg))
                .shadow_lg()
                .children(presets.into_iter().map(|zoom| {
                    let active = (self.viewport.zoom - zoom).abs() < 0.01;
                    self.render_zoom_menu_item(zoom, active, palette, cx)
                }))
                .into_any_element(),
        )
    }

    fn render_zoom_menu_item(
        &self,
        zoom: f32,
        active: bool,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id(format!("zoom-preset-{:.0}", zoom * 100.0))
            .w_full()
            .h(px(ZOOM_MENU_ITEM_HEIGHT))
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .text_sm()
            .text_color(rgb(if active {
                palette.accent
            } else {
                palette.muted_text
            }))
            .hover(move |style| style.bg(rgb(palette.button_hover)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.set_zoom(zoom, cx);
                }),
            )
            .child(
                div()
                    .w(px(16.0))
                    .h(px(16.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .children(active.then(|| {
                        Icon::new(IconName::Check)
                            .size(px(14.0))
                            .text_color(rgb(palette.accent))
                    })),
            )
            .child(format!("{:.0}%", zoom * 100.0))
            .into_any_element()
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
