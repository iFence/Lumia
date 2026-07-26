use gpui::{
    div, px, rgb, AnyElement, Context, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, Rgba, StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{Icon, IconName};
use lumia_core::FitMode;

use crate::app::LumiaApp;
use crate::editing::EditMode;
use crate::i18n::{tr, TextKey};
use crate::palette::Palette;
use crate::util::format_file_size;
use crate::widgets::edit_menu_item;
use crate::STATUS_BAR_HEIGHT;

impl LumiaApp {
    pub(crate) fn render_status_bar(
        &self,
        window: &Window,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let has_image = self.viewer.has_document();
        let viewer_enabled = has_image && !self.is_viewer_blocked();
        let count = self.sibling_count();
        let current = self
            .current_image_index()
            .map(|index| index + 1)
            .unwrap_or(0);
        let dimensions = self
            .viewer
            .display_dimensions()
            .map(|(width, height)| format!("{width}x{height}"))
            .unwrap_or_else(|| "--".to_string());
        let file_size = self
            .loads
            .file_metadata()
            .map(|metadata| format_file_size(metadata.size_bytes))
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
            .bg(Rgba {
                r: ((palette.toolbar_bg >> 16) & 0xff) as f32 / 255.0,
                g: ((palette.toolbar_bg >> 8) & 0xff) as f32 / 255.0,
                b: (palette.toolbar_bg & 0xff) as f32 / 255.0,
                a: palette.toolbar_bg_alpha,
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(self.render_status_icon_button(
                        "status-prev-image",
                        IconName::ChevronLeft,
                        viewer_enabled && current > 1,
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
                        viewer_enabled && current < count,
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
                        viewer_enabled,
                        palette,
                        cx,
                        |this, _, window, cx| {
                            this.rotate_display(3, window, cx);
                        },
                    ))
                    .child(self.render_status_icon_button(
                        "status-rotate-clockwise",
                        IconName::Redo2,
                        viewer_enabled,
                        palette,
                        cx,
                        |this, _, window, cx| {
                            this.rotate_display(1, window, cx);
                        },
                    ))
                    .child(self.render_status_text(file_size, palette))
                    .child(self.render_dimensions_button(dimensions, has_image, palette, cx)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(self.render_status_icon_button(
                        "status-fit-toggle",
                        if self.viewer.viewport().fit_mode == FitMode::FitToWindow {
                            Icon::default().path("custom/fit-to-window.svg")
                        } else {
                            Icon::default().path("custom/actual-size.svg")
                        },
                        viewer_enabled,
                        palette,
                        cx,
                        |this, _, window, cx| {
                            this.toggle_fit_or_actual_size(window, cx);
                        },
                    ))
                    .child(self.render_status_zoom_button(
                        window,
                        viewer_enabled,
                        palette,
                        cx,
                        |this, _, _, cx| {
                            this.toggle_zoom_menu(cx);
                        },
                    ))
                    .child(self.render_status_icon_button(
                        "status-zoom-in",
                        IconName::Plus,
                        viewer_enabled,
                        palette,
                        cx,
                        |this, _, window, cx| {
                            this.zoom_in_view(window, cx);
                        },
                    ))
                    .child(self.render_status_icon_button(
                        "status-zoom-out",
                        IconName::Minus,
                        viewer_enabled,
                        palette,
                        cx,
                        |this, _, window, cx| {
                            this.zoom_out_view(window, cx);
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
        window: &Window,
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
            .h(px(24.0))
            .px_2()
            .flex()
            .items_center()
            .justify_center()
            .gap_1()
            .rounded_sm()
            .border_1()
            .border_color(rgb(palette.border))
            .text_sm()
            .text_color(rgb(text_color))
            .hover(move |style| style.bg(rgb(palette.status_hover)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event, window, cx| {
                    if enabled {
                        on_click(this, event, window, cx);
                    }
                }),
            )
            .child(format!(
                "{:.0}%",
                self.image_display_scale(window).unwrap_or(1.0) * 100.0
            ))
            .child(
                Icon::new(if self.ui.show_zoom_menu {
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
        icon: impl Into<Icon>,
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
            .w(px(28.0))
            .h(px(24.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_sm()
            .hover(move |style| style.bg(rgb(palette.status_hover)))
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

    fn render_dimensions_button(
        &self,
        dimensions: String,
        has_image: bool,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let handle = self.self_handle.clone();
        let menu_handle = self.self_handle.clone();
        let menu_move_handle = self.self_handle.clone();
        let enabled = has_image && self.can_edit_current_image();
        let language = self.settings.language;
        div()
            .id("status-dimensions")
            .relative()
            .h(px(24.0))
            .px_2()
            .flex()
            .items_center()
            .gap_1()
            .rounded_sm()
            .text_sm()
            .text_color(rgb(palette.muted_text))
            .hover(move |style| style.bg(rgb(palette.status_hover)))
            .on_hover(move |hovered, _, cx| {
                let _ = handle.update(cx, |this, cx| {
                    this.set_edit_menu_hover(*hovered, has_image, cx);
                });
            })
            .child(format!("{dimensions} px"))
            .child(
                Icon::new(if self.editing.show_menu {
                    IconName::ChevronDown
                } else {
                    IconName::ChevronUp
                })
                .size(px(12.0))
                .text_color(rgb(palette.muted_text)),
            )
            .children(self.editing.show_menu.then(|| {
                div()
                    .id("status-dimensions-menu")
                    .absolute()
                    .left_0()
                    .bottom(px(24.0))
                    .w(px(184.0))
                    .p_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(palette.border))
                    .bg(rgb(palette.panel_bg))
                    .shadow_lg()
                    .on_hover(move |hovered, _, cx| {
                        let _ = menu_handle.update(cx, |this, cx| {
                            this.set_edit_menu_hover(*hovered, has_image, cx);
                        });
                    })
                    .on_mouse_move(move |_, _, cx| {
                        let _ = menu_move_handle.update(cx, |this, cx| {
                            this.set_edit_menu_hover(true, has_image, cx);
                        });
                    })
                    .child(edit_menu_item(
                        "edit-menu-crop",
                        tr(language, TextKey::EditCrop),
                        enabled,
                        palette,
                        cx,
                        |this, _, window, cx| {
                            this.open_edit_mode(EditMode::Crop, window, cx);
                        },
                    ))
                    .child(edit_menu_item(
                        "edit-menu-resize",
                        tr(language, TextKey::EditResize),
                        enabled,
                        palette,
                        cx,
                        |this, _, window, cx| {
                            this.open_edit_mode(EditMode::Resize, window, cx);
                        },
                    ))
                    .children((!enabled).then(|| {
                        div()
                            .px_3()
                            .py_2()
                            .border_t_1()
                            .border_color(rgb(palette.border))
                            .text_xs()
                            .text_color(rgb(palette.muted_text))
                            .child(tr(language, TextKey::EditUnavailable))
                    }))
            }))
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
}
