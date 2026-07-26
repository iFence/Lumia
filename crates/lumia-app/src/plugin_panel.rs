use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, rgb, AnyElement, Context, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Styled,
};
use gpui_component::scroll::ScrollableElement;
use gpui_component::{Icon, IconName};
use lumia_core::Language;

use crate::app::LumiaApp;
use crate::palette::Palette;
use crate::plugin_controls::plugin_icon;
use crate::plugin_state::PluginMenuItem;
use crate::{PLUGIN_PANEL_WIDTH, STATUS_BAR_HEIGHT};

impl LumiaApp {
    pub(crate) fn render_plugin_panel(
        &self,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let active = self.plugins.active.as_ref()?;
        let panel = active.panel.clone();
        let busy = active.busy;
        let language = language_code(self.settings.language);
        let handle = self.self_handle.clone();
        let close = div()
            .id("plugin-panel-close")
            .size(px(28.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_sm()
            .cursor_pointer()
            .hover(move |style| style.bg(rgb(palette.button_hover)))
            .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                let _ = handle.update(cx, |this, cx| this.close_plugin_session(cx));
            })
            .child(
                Icon::new(IconName::Close)
                    .size(px(16.0))
                    .text_color(rgb(palette.text)),
            );

        Some(
            div()
                .id("plugin-right-panel")
                .w(px(PLUGIN_PANEL_WIDTH))
                .h_full()
                .when(
                    !self.ui.status_bar_locked
                        && (self.ui.show_status_bar || self.ui.show_zoom_menu),
                    |panel| panel.pb(px(STATUS_BAR_HEIGHT)),
                )
                .flex_none()
                .flex()
                .flex_col()
                .border_l_1()
                .border_color(rgb(palette.border))
                .bg(rgb(palette.panel_bg))
                .child(
                    div()
                        .h(px(52.0))
                        .flex_none()
                        .px_4()
                        .flex()
                        .items_center()
                        .border_b_1()
                        .border_color(rgb(palette.border))
                        .child(
                            div()
                                .flex_1()
                                .text_sm()
                                .child(panel.title.resolve(language).to_string()),
                        )
                        .child(close),
                )
                .child(
                    div()
                        .flex_1()
                        .overflow_y_scrollbar()
                        .p_4()
                        .flex()
                        .flex_col()
                        .gap_5()
                        .children(panel.sections.into_iter().map(|section| {
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .children(section.title.map(|title| {
                                    div()
                                        .text_xs()
                                        .text_color(rgb(palette.muted_text))
                                        .child(title.resolve(language).to_string())
                                }))
                                .children(section.controls.into_iter().map(|control| {
                                    self.render_plugin_control(control, busy, palette, cx)
                                }))
                        })),
                )
                .children(self.plugins.feedback.as_ref().map(|feedback| {
                    div()
                        .px_4()
                        .pb_3()
                        .text_xs()
                        .text_color(rgb(palette.error_text))
                        .child(feedback.clone())
                }))
                .into_any_element(),
        )
    }

    pub(crate) fn render_plugin_context_menu_items(
        &self,
        items: Vec<PluginMenuItem>,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        items
            .into_iter()
            .map(|item| {
                let id = format!("plugin-menu-{}-{}", item.plugin_id, item.command_id);
                let plugin_id = item.plugin_id;
                let command_id = item.command_id;
                div()
                    .id(id)
                    .w_full()
                    .h(px(crate::widgets::CONTEXT_MENU_ITEM_HEIGHT))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_color(rgb(if item.enabled {
                        palette.text
                    } else {
                        palette.muted_text
                    }))
                    .when(item.enabled, |row| {
                        row.cursor_pointer()
                            .hover(move |style| style.bg(rgb(palette.button_hover)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    cx.stop_propagation();
                                    this.activate_plugin_command(
                                        plugin_id.clone(),
                                        command_id.clone(),
                                        window,
                                        cx,
                                    );
                                }),
                            )
                    })
                    .child(plugin_icon(item.icon, palette, item.icon_path))
                    .child(item.label)
                    .into_any_element()
            })
            .collect()
    }
}

pub(crate) fn language_code(language: Language) -> &'static str {
    match language {
        Language::English => "en",
        Language::Chinese => "zh-CN",
    }
}
