use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, rgb, svg, AnyElement, Context, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Styled,
};
use gpui_component::{Icon, IconName};
use lumia_plugin_api::{PanelControl, PluginIcon, UiValue};

use crate::app::LumiaApp;
use crate::palette::Palette;
use crate::plugin_panel::language_code;

impl LumiaApp {
    pub(crate) fn render_plugin_control(
        &self,
        control: PanelControl,
        panel_busy: bool,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let language = language_code(self.settings.language);
        match control {
            PanelControl::Button {
                id,
                label,
                icon,
                enabled,
            } => {
                let active = enabled && !panel_busy;
                let click_id = id.clone();
                div()
                    .id(format!("plugin-control-{id}"))
                    .h(px(32.0))
                    .px_3()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(palette.border))
                    .text_sm()
                    .text_color(rgb(if active {
                        palette.text
                    } else {
                        palette.muted_text
                    }))
                    .when(active, |button| {
                        button
                            .cursor_pointer()
                            .hover(move |style| style.bg(rgb(palette.button_hover)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    this.handle_plugin_button(&click_id, window, cx);
                                }),
                            )
                    })
                    .child(plugin_icon(icon, palette, None))
                    .child(label.resolve(language).to_string())
                    .into_any_element()
            }
            PanelControl::Toggle {
                id,
                label,
                value,
                enabled,
            } => self.render_plugin_value_button(
                id,
                label.resolve(language).to_string(),
                if value { "On" } else { "Off" }.to_string(),
                enabled && !panel_busy,
                UiValue::Bool(!value),
                palette,
                cx,
            ),
            PanelControl::Select {
                id,
                label,
                options,
                selected,
                enabled,
            } => {
                let enabled = enabled && !panel_busy;
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(palette.muted_text))
                            .child(label.resolve(language).to_string()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .children(options.into_iter().map(|option| {
                                let control_id = id.clone();
                                let value = option.value.clone();
                                let active = option.value == selected;
                                let asset_path = option.icon.as_ref().and_then(|icon| match icon {
                                    PluginIcon::Asset(asset_id) => {
                                        self.plugins.active_asset_path(asset_id)
                                    }
                                    _ => None,
                                });
                                div()
                                    .id(format!("plugin-select-{id}-{}", option.value))
                                    .size(px(44.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgb(if active {
                                        palette.accent
                                    } else {
                                        palette.border
                                    }))
                                    .when(enabled, |button| {
                                        button
                                            .cursor_pointer()
                                            .hover(move |style| style.bg(rgb(palette.button_hover)))
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(move |this, _, _, cx| {
                                                    this.dispatch_plugin_ui_event(
                                                        control_id.clone(),
                                                        UiValue::String(value.clone()),
                                                        cx,
                                                    );
                                                }),
                                            )
                                    })
                                    .child(plugin_icon(
                                        option.icon.unwrap_or(PluginIcon::Annotation),
                                        palette,
                                        asset_path,
                                    ))
                            })),
                    )
                    .into_any_element()
            }
            PanelControl::Slider {
                id,
                label,
                value,
                min,
                max,
                step,
                enabled,
            } => {
                let decrement = (value - step).max(min);
                let increment = (value + step).min(max);
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(palette.muted_text))
                            .child(label.resolve(language).to_string()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(self.plugin_step_button(
                                format!("{id}-decrement"),
                                "−",
                                id.clone(),
                                decrement,
                                enabled && !panel_busy && value > min,
                                palette,
                                cx,
                            ))
                            .child(
                                div()
                                    .flex_1()
                                    .text_center()
                                    .text_sm()
                                    .child(format!("{value:.1}")),
                            )
                            .child(self.plugin_step_button(
                                format!("{id}-increment"),
                                "+",
                                id,
                                increment,
                                enabled && !panel_busy && value < max,
                                palette,
                                cx,
                            )),
                    )
                    .into_any_element()
            }
            PanelControl::Color {
                id,
                label,
                value,
                enabled,
            } => {
                let colors = ["#ff3b30", "#ff9500", "#34c759", "#007aff", "#af52de"];
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(palette.muted_text))
                            .child(label.resolve(language).to_string()),
                    )
                    .child(div().flex().gap_2().children(colors.map(|color| {
                        let control_id = id.clone();
                        div()
                            .id(format!("plugin-color-{color}"))
                            .size(px(28.0))
                            .rounded_full()
                            .border_2()
                            .border_color(rgb(if color == value {
                                palette.text
                            } else {
                                palette.border
                            }))
                            .bg(rgb(parse_color(color).unwrap_or(0)))
                            .when(enabled && !panel_busy, |swatch| {
                                swatch.cursor_pointer().on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |this, _, _, cx| {
                                        this.dispatch_plugin_ui_event(
                                            control_id.clone(),
                                            UiValue::String(color.to_string()),
                                            cx,
                                        );
                                    }),
                                )
                            })
                    })))
                    .into_any_element()
            }
            PanelControl::Text {
                label,
                value,
                enabled: _,
                ..
            } => div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(palette.muted_text))
                        .child(label.resolve(language).to_string()),
                )
                .child(div().text_sm().child(value))
                .into_any_element(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn render_plugin_value_button(
        &self,
        id: String,
        label: String,
        value_label: String,
        enabled: bool,
        value: UiValue,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let control_id = id.clone();
        div()
            .id(format!("plugin-control-{id}"))
            .h(px(32.0))
            .px_3()
            .flex()
            .items_center()
            .justify_between()
            .rounded_md()
            .border_1()
            .border_color(rgb(palette.border))
            .when(enabled, |button| {
                button
                    .cursor_pointer()
                    .hover(move |style| style.bg(rgb(palette.button_hover)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.dispatch_plugin_ui_event(control_id.clone(), value.clone(), cx);
                        }),
                    )
            })
            .child(label)
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(palette.muted_text))
                    .child(value_label),
            )
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn plugin_step_button(
        &self,
        id: String,
        label: &'static str,
        control_id: String,
        value: f32,
        enabled: bool,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id(id)
            .size(px(28.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_sm()
            .border_1()
            .border_color(rgb(palette.border))
            .when(enabled, |button| {
                button
                    .cursor_pointer()
                    .hover(move |style| style.bg(rgb(palette.button_hover)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.dispatch_plugin_ui_event(
                                control_id.clone(),
                                UiValue::Number(value),
                                cx,
                            );
                        }),
                    )
            })
            .child(label)
            .into_any_element()
    }
}

pub(crate) fn plugin_icon(
    icon: PluginIcon,
    palette: Palette,
    external_path: Option<std::path::PathBuf>,
) -> AnyElement {
    if let Some(path) = external_path {
        return svg()
            .external_path(path.to_string_lossy().to_string())
            .size(px(20.0))
            .text_color(rgb(palette.text))
            .into_any_element();
    }
    let icon = match icon {
        PluginIcon::Annotation => IconName::Palette,
        PluginIcon::Select => IconName::Frame,
        PluginIcon::Text => IconName::CaseSensitive,
        PluginIcon::Rectangle => IconName::Frame,
        PluginIcon::Ellipse => IconName::Asterisk,
        PluginIcon::Arrow => IconName::ArrowRight,
        PluginIcon::Undo => IconName::Undo2,
        PluginIcon::Redo => IconName::Redo2,
        PluginIcon::Export => IconName::ExternalLink,
        PluginIcon::Asset(_) => IconName::Plus,
    };
    Icon::new(icon)
        .size(px(16.0))
        .text_color(rgb(palette.text))
        .into_any_element()
}

fn parse_color(value: &str) -> Option<u32> {
    u32::from_str_radix(value.strip_prefix('#')?, 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_color_parser_accepts_css_hex() {
        assert_eq!(parse_color("#ff3b30"), Some(0xff3b30));
        assert_eq!(parse_color("ff3b30"), None);
    }
}
