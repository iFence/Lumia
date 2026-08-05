use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, rgb, AnyElement, Context, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, Rgba, StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{tooltip::Tooltip, Icon, IconName};
use lumia_core::FitMode;

use crate::app::LumiaApp;
use crate::editing::EditMode;
use crate::i18n::{tr, TextKey};
use crate::palette::Palette;
use crate::util::format_file_size;
use crate::widgets::edit_menu_item;
use crate::{
    EDIT_MENU_WIDTH, STATUS_BAR_HEIGHT, STATUS_CONTROL_HEIGHT, STATUS_MENU_CONTROL_OFFSET,
    ZOOM_BUTTON_WIDTH,
};

impl LumiaApp {
    /// The status bar stays rendered while the zoom or edit menu is open, so
    /// moving the pointer toward those menus does not hide it.
    pub(crate) fn status_bar_visible(&self) -> bool {
        self.ui.status_bar_locked
            || self.ui.show_status_bar
            || self.ui.show_zoom_menu
            || self.editing.show_menu
    }

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
            .when(!self.ui.status_bar_locked, |bar| {
                bar.absolute().left_0().right_0().bottom_0()
            })
            .flex_none()
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
                    .child(self.render_dimensions_button(dimensions, has_image, palette, cx))
                    .when(self.current_gps_coordinates().is_some(), |controls| {
                        controls.child(self.render_status_location_button(
                            viewer_enabled,
                            palette,
                            cx,
                        ))
                    }),
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
                    ))
                    .child(self.render_status_lock_button(palette, cx)),
            )
            .children(self.render_zoom_menu(palette, window, cx))
    }

    fn render_status_lock_button(&self, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        let locked = self.ui.status_bar_locked;
        let language = self.settings.language;
        let tooltip = tr(
            language,
            if locked {
                TextKey::UnlockStatusBar
            } else {
                TextKey::LockStatusBar
            },
        );
        let icon_path = if locked {
            "custom/status-bar-lock.svg"
        } else {
            "custom/status-bar-unlock.svg"
        };

        div()
            .id("status-bar-lock")
            .w(px(28.0))
            .h(px(24.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_sm()
            .cursor_pointer()
            .hover(move |style| style.bg(rgb(palette.status_hover)))
            .tooltip(move |window, cx| Tooltip::new(tooltip).build(window, cx))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.ui.status_bar_locked = !this.ui.status_bar_locked;
                    this.ui.show_status_bar = true;
                    this.ui.context_menu_position = None;
                    cx.notify();
                }),
            )
            .child(
                Icon::default()
                    .path(icon_path)
                    .size(px(16.0))
                    .text_color(rgb(palette.text)),
            )
            .into_any_element()
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
        let self_handle = self.self_handle.clone();
        let button = div()
            .id("status-zoom-menu-button")
            .w(px(ZOOM_BUTTON_WIDTH))
            .h(px(STATUS_CONTROL_HEIGHT))
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
            );
        // Track the zoom button's on-screen bounds so the zoom menu can be
        // centered under it and kept open while the pointer hovers it.
        div()
            .flex()
            .items_center()
            .on_children_prepainted(move |bounds, _, cx| {
                if let Some(bounds) = bounds.first() {
                    let _ = self_handle.update(cx, |this, _| {
                        this.ui.zoom_menu_anchor = Some(*bounds);
                    });
                }
            })
            .child(button)
            .into_any_element()
    }

    fn render_status_location_button(
        &self,
        enabled: bool,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tooltip = tr(self.settings.language, TextKey::OpenImageLocation);
        let icon_color = if enabled {
            palette.text
        } else {
            palette.muted_text
        };

        div()
            .id("status-image-location")
            .w(px(28.0))
            .h(px(24.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_sm()
            .hover(move |style| style.bg(rgb(palette.status_hover)))
            .tooltip(move |window, cx| Tooltip::new(tooltip).build(window, cx))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, _| {
                    if enabled {
                        this.open_current_image_location();
                    }
                }),
            )
            .child(
                Icon::default()
                    .path("custom/map-pin.svg")
                    .size(px(16.0))
                    .text_color(rgb(icon_color)),
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
        let enabled = has_image && self.can_edit_current_image();
        let language = self.settings.language;
        let self_handle = self.self_handle.clone();
        // Center the popup under the narrow dimensions button instead of
        // anchoring it to the button's left edge, which pushed it to the right.
        let menu_left = self.editing.menu_anchor.map_or(0.0, |anchor| {
            (f32::from(anchor.size.width) - EDIT_MENU_WIDTH) / 2.0
        });
        let button = div()
            .id("status-dimensions")
            .relative()
            .h(px(STATUS_CONTROL_HEIGHT))
            .px_2()
            .flex()
            .items_center()
            .rounded_sm()
            .text_sm()
            .text_color(rgb(palette.muted_text))
            .hover(move |style| style.bg(rgb(palette.status_hover)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    if has_image {
                        this.editing.show_menu = !this.editing.show_menu;
                        this.ui.show_zoom_menu = false;
                        cx.notify();
                    }
                }),
            )
            .child(dimensions)
            .children(self.editing.show_menu.then(|| {
                div()
                    .id("status-dimensions-menu")
                    .absolute()
                    .left(px(menu_left))
                    .bottom(px(STATUS_MENU_CONTROL_OFFSET))
                    .w(px(EDIT_MENU_WIDTH))
                    .p_1()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(palette.border))
                    .bg(rgb(palette.panel_bg))
                    .shadow_lg()
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
            }));
        // Track the button's on-screen bounds so the root mouse-move handler
        // can keep the edit menu open while the pointer hovers it, matching
        // the zoom menu's keep-open zone.
        div()
            .flex()
            .items_center()
            .on_children_prepainted(move |bounds, _, cx| {
                if let Some(bounds) = bounds.first() {
                    let _ = self_handle.update(cx, |this, _| {
                        this.editing.menu_anchor = Some(*bounds);
                    });
                }
            })
            .child(button)
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
