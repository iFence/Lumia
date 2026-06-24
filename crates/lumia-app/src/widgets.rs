use gpui::{
    div, prelude::FluentBuilder, px, rgb, AnyElement, ClickEvent, Context, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, ParentElement, SharedString,
    StatefulInteractiveElement, Styled, Window,
};

use crate::app::LumiaApp;
use crate::palette::Palette;
use crate::TITLE_BAR_HEIGHT;

pub(crate) fn toolbar_button(
    id: &'static str,
    label: &'static str,
    palette: Palette,
    cx: &mut Context<LumiaApp>,
    on_click: impl Fn(&mut LumiaApp, &ClickEvent, &mut Window, &mut Context<LumiaApp>) + 'static,
) -> AnyElement {
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_md()
        .bg(rgb(palette.button_bg))
        .text_color(rgb(palette.text))
        .text_sm()
        .cursor_pointer()
        .hover(move |style| style.bg(rgb(palette.button_hover)))
        .on_click(cx.listener(on_click))
        .child(label)
        .into_any_element()
}

pub(crate) fn context_menu_item(
    id: &'static str,
    label: &'static str,
    palette: Palette,
    cx: &mut Context<LumiaApp>,
    on_click: impl Fn(&mut LumiaApp, &MouseDownEvent, &mut Window, &mut Context<LumiaApp>) + 'static,
) -> AnyElement {
    div()
        .id(id)
        .w_full()
        .px_3()
        .py_1()
        .cursor_pointer()
        .hover(move |style| style.bg(rgb(palette.button_hover)))
        .on_mouse_down(MouseButton::Left, cx.listener(on_click))
        .child(label)
        .into_any_element()
}

pub(crate) fn settings_group_button(
    id: &'static str,
    label: &'static str,
    active: bool,
    palette: Palette,
    cx: &mut Context<LumiaApp>,
    on_click: impl Fn(&mut LumiaApp, &ClickEvent, &mut Window, &mut Context<LumiaApp>) + 'static,
) -> AnyElement {
    div()
        .id(id)
        .w_full()
        .px_3()
        .py_2()
        .rounded_md()
        .text_sm()
        .cursor_pointer()
        .when(active, move |style| {
            style
                .bg(rgb(palette.selection_bg))
                .text_color(rgb(palette.text))
        })
        .when(!active, move |style| {
            style
                .bg(rgb(palette.sidebar_bg))
                .text_color(rgb(palette.muted_text))
        })
        .hover(move |style| style.bg(rgb(palette.button_hover)))
        .on_click(cx.listener(on_click))
        .child(label)
        .into_any_element()
}

pub(crate) fn settings_option_button(
    id: &'static str,
    label: &'static str,
    active: bool,
    palette: Palette,
    cx: &mut Context<LumiaApp>,
    on_click: impl Fn(&mut LumiaApp, &ClickEvent, &mut Window, &mut Context<LumiaApp>) + 'static,
) -> AnyElement {
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(rgb(if active {
            palette.selection_bg
        } else {
            palette.border
        }))
        .text_sm()
        .cursor_pointer()
        .when(active, move |style| {
            style
                .bg(rgb(palette.selection_bg))
                .text_color(rgb(palette.text))
        })
        .when(!active, move |style| {
            style
                .bg(rgb(palette.panel_bg))
                .text_color(rgb(palette.muted_text))
        })
        .hover(move |style| style.bg(rgb(palette.button_hover)))
        .on_click(cx.listener(on_click))
        .child(label)
        .into_any_element()
}

pub(crate) fn settings_label(
    title: &'static str,
    description: &'static str,
    palette: Palette,
) -> impl IntoElement {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_sm().child(title))
        .child(
            div()
                .text_xs()
                .text_color(rgb(palette.muted_text))
                .child(description),
        )
}

pub(crate) fn shortcut_record_button(
    id: &'static str,
    current_binding: String,
    is_recording: bool,
    palette: Palette,
    cx: &mut Context<LumiaApp>,
    on_click: impl Fn(&mut LumiaApp, &ClickEvent, &mut Window, &mut Context<LumiaApp>) + 'static,
) -> AnyElement {
    let text: SharedString = if is_recording {
        "...".into()
    } else {
        current_binding.into()
    };

    div()
        .id(id)
        .min_w(px(120.0))
        .px_3()
        .py_1()
        .rounded_md()
        .border_1()
        .text_sm()
        .cursor_pointer()
        .when(is_recording, move |style| {
            style
                .border_color(rgb(palette.accent))
                .bg(rgb(palette.accent_bg))
                .text_color(rgb(palette.text))
        })
        .when(!is_recording, move |style| {
            style
                .border_color(rgb(palette.border))
                .bg(rgb(palette.subtle_bg))
                .text_color(rgb(palette.muted_text))
        })
        .hover(move |style| style.bg(rgb(palette.button_hover)))
        .on_click(cx.listener(on_click))
        .child(text)
        .into_any_element()
}

pub(crate) fn shortcut_reset_button(
    id: &'static str,
    label: &'static str,
    palette: Palette,
    cx: &mut Context<LumiaApp>,
    on_click: impl Fn(&mut LumiaApp, &ClickEvent, &mut Window, &mut Context<LumiaApp>) + 'static,
) -> AnyElement {
    div()
        .id(id)
        .px_2()
        .py_1()
        .rounded_md()
        .text_xs()
        .cursor_pointer()
        .text_color(rgb(palette.muted_text))
        .hover(move |style| {
            style
                .bg(rgb(palette.button_hover))
                .text_color(rgb(palette.text))
        })
        .on_click(cx.listener(on_click))
        .child(label)
        .into_any_element()
}

pub(crate) fn titlebar_button(
    id: &'static str,
    symbol: &'static str,
    palette: Palette,
    cx: &mut Context<LumiaApp>,
    on_click: impl Fn(&mut LumiaApp, &ClickEvent, &mut Window, &mut Context<LumiaApp>) + 'static,
) -> AnyElement {
    div()
        .id(id)
        .w(px(36.0))
        .h(px(TITLE_BAR_HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .rounded_none()
        .cursor_pointer()
        .text_color(rgb(palette.muted_text))
        .text_sm()
        .hover(move |style| style.bg(rgb(palette.button_hover)).text_color(rgb(palette.text)))
        .on_click(cx.listener(on_click))
        .child(symbol)
        .into_any_element()
}

pub(crate) fn titlebar_close_button(
    id: &'static str,
    symbol: &'static str,
    palette: Palette,
    cx: &mut Context<LumiaApp>,
    on_click: impl Fn(&mut LumiaApp, &ClickEvent, &mut Window, &mut Context<LumiaApp>) + 'static,
) -> AnyElement {
    div()
        .id(id)
        .w(px(36.0))
        .h(px(TITLE_BAR_HEIGHT))
        .flex()
        .items_center()
        .justify_center()
        .rounded_none()
        .cursor_pointer()
        .text_color(rgb(palette.muted_text))
        .text_sm()
        .hover(move |style| style.bg(rgb(0xe81123)).text_color(rgb(0xffffff)))
        .on_click(cx.listener(on_click))
        .child(symbol)
        .into_any_element()
}
