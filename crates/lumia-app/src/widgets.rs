use gpui::{
    div, px, rgb, Context, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, SharedString, StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{
    button::{Button, ButtonVariants, DropdownButton},
    menu::PopupMenu,
    Selectable, Sizable,
};

use crate::app::LumiaApp;
use crate::palette::Palette;

pub(crate) fn context_menu_item(
    id: &'static str,
    label: &'static str,
    palette: Palette,
    cx: &mut Context<LumiaApp>,
    on_click: impl Fn(&mut LumiaApp, &MouseDownEvent, &mut Window, &mut Context<LumiaApp>) + 'static,
) -> gpui::AnyElement {
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
    on_click: impl Fn(&mut LumiaApp, &gpui::ClickEvent, &mut Window, &mut Context<LumiaApp>) + 'static,
) -> gpui::AnyElement {
    let label_color = if active {
        palette.text
    } else {
        palette.muted_text
    };
    let row_bg = if active {
        rgb(palette.accent_soft)
    } else {
        rgb(palette.sidebar_bg).opacity(0.0)
    };
    let hover_bg = if active {
        rgb(palette.accent_soft)
    } else {
        rgb(palette.button_hover)
    };
    let indicator_bg = if active {
        rgb(palette.accent)
    } else {
        rgb(palette.sidebar_bg).opacity(0.0)
    };

    div()
        .id(id)
        .w_full()
        .px_2()
        .py_1()
        .rounded_md()
        .cursor_pointer()
        .bg(row_bg)
        .text_color(rgb(label_color))
        .hover(move |style| style.bg(hover_bg).text_color(rgb(palette.text)))
        .on_click(cx.listener(on_click))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py_1()
                .child(div().w(px(3.0)).h(px(14.0)).rounded_sm().bg(indicator_bg))
                .child(div().text_sm().child(label)),
        )
        .into_any_element()
}

pub(crate) fn settings_dropdown_button(
    id: &'static str,
    label: &'static str,
    menu: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
) -> gpui::AnyElement {
    DropdownButton::new(id)
        .button(
            Button::new(format!("{id}-trigger"))
                .label(label)
                .min_w(px(148.0)),
        )
        .outline()
        .small()
        .compact()
        .dropdown_menu(menu)
        .into_any_element()
}

pub(crate) fn settings_label(
    title: &'static str,
    description: &'static str,
    palette: Palette,
) -> impl gpui::IntoElement {
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
    _palette: Palette,
    cx: &mut Context<LumiaApp>,
    on_click: impl Fn(&mut LumiaApp, &gpui::ClickEvent, &mut Window, &mut Context<LumiaApp>) + 'static,
) -> gpui::AnyElement {
    let text: SharedString = if is_recording {
        "...".into()
    } else {
        current_binding.into()
    };
    let app = cx.weak_entity();

    Button::new(id)
        .outline()
        .small()
        .selected(is_recording)
        .min_w(px(120.0))
        .label(text)
        .on_click(move |event, window, cx| {
            let _ = app.update(cx, |this, cx| on_click(this, event, window, cx));
        })
        .into_any_element()
}

pub(crate) fn shortcut_reset_button(
    id: &'static str,
    label: &'static str,
    _palette: Palette,
    cx: &mut Context<LumiaApp>,
    on_click: impl Fn(&mut LumiaApp, &gpui::ClickEvent, &mut Window, &mut Context<LumiaApp>) + 'static,
) -> gpui::AnyElement {
    let app = cx.weak_entity();

    Button::new(id)
        .text()
        .xsmall()
        .label(label)
        .on_click(move |event, window, cx| {
            let _ = app.update(cx, |this, cx| on_click(this, event, window, cx));
        })
        .into_any_element()
}
