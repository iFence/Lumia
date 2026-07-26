use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, rgb, Context, InteractiveElement, IntoElement, Keystroke, MouseButton, MouseDownEvent,
    ParentElement, StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{button::ButtonVariants as _, Disableable as _};
use gpui_component::{
    button::{Button, ButtonRounded},
    kbd::Kbd,
    menu::{DropdownMenu, PopupMenu},
};

use crate::app::LumiaApp;
use crate::palette::Palette;

pub(crate) const CONTEXT_MENU_ITEM_HEIGHT: f32 = 28.0;

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
        .h(px(CONTEXT_MENU_ITEM_HEIGHT))
        .px_3()
        .flex()
        .items_center()
        .cursor_pointer()
        .hover(move |style| style.bg(rgb(palette.button_hover)))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event, window, cx| {
                cx.stop_propagation();
                on_click(this, event, window, cx);
            }),
        )
        .child(label)
        .into_any_element()
}

pub(crate) fn edit_menu_item(
    id: &'static str,
    label: &'static str,
    enabled: bool,
    palette: Palette,
    cx: &mut Context<LumiaApp>,
    on_click: impl Fn(&mut LumiaApp, &MouseDownEvent, &mut Window, &mut Context<LumiaApp>) + 'static,
) -> gpui::AnyElement {
    div()
        .id(id)
        .w_full()
        .h(px(30.0))
        .px_3()
        .flex()
        .items_center()
        .rounded_sm()
        .text_sm()
        .text_color(rgb(if enabled {
            palette.text
        } else {
            palette.muted_text
        }))
        .when(enabled, |item| {
            item.cursor_pointer()
                .hover(move |style| style.bg(rgb(palette.button_hover)))
                .on_mouse_down(MouseButton::Left, cx.listener(on_click))
        })
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
    Button::new(format!("{id}-trigger"))
        .w(px(172.0))
        .h(px(36.0))
        .px_3()
        .label(label)
        .outline()
        .rounded(ButtonRounded::Large)
        .dropdown_caret(true)
        .dropdown_menu(move |popup, window, cx| menu(popup.min_w(px(172.0)), window, cx))
        .into_any_element()
}

pub(crate) fn settings_action_button(
    id: &'static str,
    label: &'static str,
    primary: bool,
    disabled: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> gpui::AnyElement {
    let button = Button::new(id)
        .h(px(32.0))
        .px_3()
        .label(label)
        .disabled(disabled)
        .on_click(on_click);
    if primary {
        button.primary().into_any_element()
    } else {
        button.outline().into_any_element()
    }
}

pub(crate) fn edit_option_button(
    id: &'static str,
    label: &'static str,
    active: bool,
    palette: Palette,
    cx: &mut Context<LumiaApp>,
    on_click: impl Fn(&mut LumiaApp, &gpui::ClickEvent, &mut Window, &mut Context<LumiaApp>) + 'static,
) -> gpui::AnyElement {
    div()
        .id(id)
        .px_2()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(rgb(if active {
            palette.accent
        } else {
            palette.border
        }))
        .bg(rgb(if active {
            palette.accent_soft
        } else {
            palette.subtle_bg
        }))
        .text_xs()
        .text_color(rgb(if active {
            palette.text
        } else {
            palette.muted_text
        }))
        .cursor_pointer()
        .hover(move |style| style.bg(rgb(palette.button_hover)))
        .on_click(cx.listener(on_click))
        .child(label)
        .into_any_element()
}

pub(crate) fn settings_label(title: &'static str) -> impl gpui::IntoElement {
    div().flex_1().text_sm().child(title)
}

pub(crate) fn shortcut_record_button(
    id: &'static str,
    current_binding: String,
    is_recording: bool,
    cx: &mut Context<LumiaApp>,
    on_click: impl Fn(&mut LumiaApp, &gpui::ClickEvent, &mut Window, &mut Context<LumiaApp>) + 'static,
) -> gpui::AnyElement {
    let button = Button::new(id)
        .min_w(px(132.0))
        .h(px(32.0))
        .px_3()
        .on_click(cx.listener(on_click));

    if is_recording {
        button.primary().label("…").into_any_element()
    } else {
        match Keystroke::parse(&current_binding) {
            Ok(stroke) => button
                .ghost()
                .child(Kbd::new(stroke).outline())
                .into_any_element(),
            Err(_) => button.ghost().label(current_binding).into_any_element(),
        }
    }
}

pub(crate) fn shortcut_reset_button(
    id: &'static str,
    label: &'static str,
    cx: &mut Context<LumiaApp>,
    on_click: impl Fn(&mut LumiaApp, &gpui::ClickEvent, &mut Window, &mut Context<LumiaApp>) + 'static,
) -> gpui::AnyElement {
    Button::new(id)
        .label(label)
        .ghost()
        .compact()
        .on_click(cx.listener(on_click))
        .into_any_element()
}
