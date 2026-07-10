use gpui::{
    div, px, rgb, AnyElement, Context, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, Styled, Window,
};
use gpui_component::{Icon, IconName};
use lumia_core::FitMode;

use crate::app::LumiaApp;
use crate::palette::Palette;
use crate::util::format_file_size;
use crate::STATUS_BAR_HEIGHT;

impl LumiaApp {
    pub(crate) fn render_status_bar(
        &self,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let has_image = self.viewer.has_document();
        let count = self.sibling_count();
        let current = self
            .current_image_index()
            .map(|index| index + 1)
            .unwrap_or(0);
        let dimensions = self
            .viewer
            .document()
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
                        if self.viewer.viewport().fit_mode == FitMode::FitToWindow {
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
                            if this.viewer.has_document() {
                                this.viewer.viewport_mut().zoom_in();
                                this.ui.show_zoom_menu = false;
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
                            if this.viewer.has_document() {
                                this.viewer.viewport_mut().zoom_out();
                                this.ui.show_zoom_menu = false;
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
            .child(format!("{:.0}%", self.viewer.viewport().zoom * 100.0))
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
}
