use gpui::{
    div, point, px, rgb, AnyElement, Context, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Pixels, Point, Styled, Window,
};
use gpui_component::{Icon, IconName};

use crate::app::LumiaApp;
use crate::i18n::{tr, TextKey};
use crate::palette::Palette;
use crate::widgets::{context_menu_item, context_menu_item_enabled, CONTEXT_MENU_ITEM_HEIGHT};
use crate::{
    EDIT_PANEL_WIDTH, STATUS_BAR_HEIGHT, STATUS_MENU_BOTTOM, ZOOM_MENU_ITEM_HEIGHT,
    ZOOM_MENU_RIGHT, ZOOM_MENU_WIDTH,
};

const CONTEXT_MENU_WIDTH: f32 = 156.0;
const CONTEXT_MENU_HEIGHT: f32 = 2.0 + 8.0 + 6.0 * CONTEXT_MENU_ITEM_HEIGHT + 2.0 * 9.0;
const CONTEXT_MENU_MARGIN: f32 = 8.0;

impl LumiaApp {
    pub(crate) fn clamped_context_menu_position(
        &self,
        pointer: Point<Pixels>,
        window: &Window,
    ) -> Point<Pixels> {
        let viewport = window.viewport_size();
        let right_inset = if self.editing.mode.is_some() {
            EDIT_PANEL_WIDTH
        } else {
            0.0
        };
        let bottom_inset =
            if self.ui.status_bar_locked || self.ui.show_status_bar || self.ui.show_zoom_menu {
                STATUS_BAR_HEIGHT
            } else {
                0.0
            };
        point(
            px(clamp_menu_coordinate(
                f32::from(pointer.x),
                f32::from(viewport.width),
                CONTEXT_MENU_WIDTH,
                right_inset,
            )),
            px(clamp_menu_coordinate(
                f32::from(pointer.y),
                f32::from(viewport.height),
                CONTEXT_MENU_HEIGHT,
                bottom_inset,
            )),
        )
    }

    pub(crate) fn render_zoom_menu(
        &self,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.ui.show_zoom_menu {
            return None;
        }

        let presets = [32.0, 16.0, 8.0, 4.0, 2.0, 1.5, 1.0, 0.5, 0.1];
        Some(
            div()
                .id("status-zoom-menu")
                .absolute()
                .right(px(ZOOM_MENU_RIGHT))
                .bottom(px(STATUS_MENU_BOTTOM))
                .w(px(ZOOM_MENU_WIDTH))
                .py_2()
                .rounded_md()
                .border_1()
                .border_color(rgb(palette.border))
                .bg(rgb(palette.panel_bg))
                .shadow_lg()
                .children(presets.into_iter().map(|zoom| {
                    let active = (self.viewer.viewport().zoom - zoom).abs() < 0.01;
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
                cx.listener(move |this, _, window, cx| {
                    this.set_zoom(zoom, window, cx);
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
    pub(crate) fn render_context_menu(
        &self,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let language = self.settings.language;
        let slideshow_active = self.slideshow.is_active();
        let slideshow_enabled = slideshow_active || self.can_start_slideshow();
        let slideshow_label = if slideshow_active {
            TextKey::StopSlideshow
        } else {
            TextKey::Slideshow
        };

        self.ui.context_menu_position.map(|position| {
            div()
                .id("viewer-context-menu")
                .absolute()
                .left(position.x)
                .top(position.y)
                .w(px(CONTEXT_MENU_WIDTH))
                .py_1()
                .rounded_md()
                .bg(rgb(palette.panel_bg))
                .border_1()
                .border_color(rgb(palette.border))
                .shadow_lg()
                .text_color(rgb(palette.text))
                .text_sm()
                .child(context_menu_item(
                    "open-menu-item",
                    tr(language, TextKey::Open),
                    palette,
                    cx,
                    |this, _, _, cx| {
                        this.open_file_dialog(cx, None);
                    },
                ))
                .child(context_menu_item(
                    "open-url-menu-item",
                    tr(language, TextKey::OpenUrl),
                    palette,
                    cx,
                    |this, _, window, cx| {
                        this.open_url_dialog(window, cx);
                    },
                ))
                .child(context_menu_item_enabled(
                    "slideshow-menu-item",
                    tr(language, slideshow_label),
                    slideshow_enabled,
                    palette,
                    cx,
                    |this, _, window, cx| {
                        this.toggle_slideshow(window, cx);
                    },
                ))
                .child(div().h(px(1.0)).my_1().bg(rgb(palette.border)))
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
                        this.ui.context_menu_position = None;
                        cx.notify();
                    },
                ))
                .child(div().h(px(1.0)).my_1().bg(rgb(palette.border)))
                .child(context_menu_item(
                    "quit-menu-item",
                    tr(language, TextKey::Quit),
                    palette,
                    cx,
                    |this, _, _, cx| {
                        this.ui.context_menu_position = None;
                        cx.quit();
                    },
                ))
        })
    }
}

fn clamp_menu_coordinate(
    pointer: f32,
    viewport_extent: f32,
    menu_extent: f32,
    trailing_inset: f32,
) -> f32 {
    let maximum = (viewport_extent - trailing_inset - menu_extent - CONTEXT_MENU_MARGIN).max(0.0);
    pointer.clamp(CONTEXT_MENU_MARGIN.min(maximum), maximum)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_menu_coordinate_stays_inside_trailing_edge() {
        assert_eq!(
            clamp_menu_coordinate(790.0, 800.0, CONTEXT_MENU_WIDTH, 0.0),
            636.0
        );
        assert_eq!(
            clamp_menu_coordinate(590.0, 600.0, CONTEXT_MENU_HEIGHT, STATUS_BAR_HEIGHT),
            360.0
        );
    }

    #[test]
    fn context_menu_coordinate_preserves_safe_positions_and_small_viewports() {
        assert_eq!(
            clamp_menu_coordinate(240.0, 800.0, CONTEXT_MENU_WIDTH, 0.0),
            240.0
        );
        assert_eq!(
            clamp_menu_coordinate(10.0, 100.0, CONTEXT_MENU_WIDTH, 0.0),
            0.0
        );
    }
}
