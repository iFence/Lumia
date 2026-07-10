use gpui::{
    div, px, rgb, AnyElement, Context, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Styled,
};
use gpui_component::{Icon, IconName};

use crate::app::LumiaApp;
use crate::i18n::{tr, TextKey};
use crate::palette::Palette;
use crate::widgets::context_menu_item;
use crate::{
    STATUS_BAR_HEIGHT, ZOOM_MENU_BOTTOM_GAP, ZOOM_MENU_ITEM_HEIGHT, ZOOM_MENU_RIGHT,
    ZOOM_MENU_WIDTH,
};

impl LumiaApp {
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
                .bottom(px(STATUS_BAR_HEIGHT + ZOOM_MENU_BOTTOM_GAP))
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
    pub(crate) fn render_decoding_overlay(&self, palette: Palette) -> Option<impl IntoElement> {
        if !self.loads.is_decoding() {
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

    pub(crate) fn render_context_menu(
        &self,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let language = self.settings.language;

        self.ui.context_menu_position.map(|position| {
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
                        this.ui.context_menu_position = None;
                        cx.notify();
                    },
                ))
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
