use gpui::{
    div, prelude::FluentBuilder, rgb, AnyElement, ClickEvent, Context, InteractiveElement,
    IntoElement, MouseButton, MouseDownEvent, ParentElement, StatefulInteractiveElement, Styled,
    Window,
};

use crate::app::LumiaApp;
use crate::palette::Palette;

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
