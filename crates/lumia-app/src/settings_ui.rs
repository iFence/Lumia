use gpui::{
    div, px, rgb, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window,
};
use lumia_core::{Language, SettingsGroup, ShortcutId, ThemeAccent};

use crate::app::LumiaApp;
use crate::i18n::{tr, TextKey};
use crate::palette::Palette;
use crate::widgets::settings_group_button;

pub(crate) fn language_text_key(language: Language) -> TextKey {
    match language {
        Language::English => TextKey::English,
        Language::Chinese => TextKey::Chinese,
    }
}

pub(crate) fn theme_accent_text_key(accent: ThemeAccent) -> TextKey {
    match accent {
        ThemeAccent::Blue => TextKey::AccentBlue,
        ThemeAccent::Green => TextKey::AccentGreen,
        ThemeAccent::Orange => TextKey::AccentOrange,
        ThemeAccent::Rose => TextKey::AccentRose,
    }
}

pub(crate) fn shortcut_record_button_id(shortcut_id: ShortcutId) -> &'static str {
    match shortcut_id {
        ShortcutId::OpenFile => "shortcut-record-open-file",
        ShortcutId::ZoomIn => "shortcut-record-zoom-in",
        ShortcutId::ZoomOut => "shortcut-record-zoom-out",
        ShortcutId::ZoomFit => "shortcut-record-zoom-fit",
        ShortcutId::ToggleFullscreen => "shortcut-record-toggle-fullscreen",
        ShortcutId::ExitFullscreen => "shortcut-record-exit-fullscreen",
        ShortcutId::ToggleImageInfo => "shortcut-record-toggle-image-info",
        ShortcutId::NextImage => "shortcut-record-next-image",
        ShortcutId::PreviousImage => "shortcut-record-previous-image",
        ShortcutId::OpenSettings => "shortcut-record-open-settings",
        ShortcutId::About => "shortcut-record-about",
        ShortcutId::Quit => "shortcut-record-quit",
    }
}

pub(crate) fn shortcut_reset_button_id(shortcut_id: ShortcutId) -> &'static str {
    match shortcut_id {
        ShortcutId::OpenFile => "shortcut-reset-open-file",
        ShortcutId::ZoomIn => "shortcut-reset-zoom-in",
        ShortcutId::ZoomOut => "shortcut-reset-zoom-out",
        ShortcutId::ZoomFit => "shortcut-reset-zoom-fit",
        ShortcutId::ToggleFullscreen => "shortcut-reset-toggle-fullscreen",
        ShortcutId::ExitFullscreen => "shortcut-reset-exit-fullscreen",
        ShortcutId::ToggleImageInfo => "shortcut-reset-toggle-image-info",
        ShortcutId::NextImage => "shortcut-reset-next-image",
        ShortcutId::PreviousImage => "shortcut-reset-previous-image",
        ShortcutId::OpenSettings => "shortcut-reset-open-settings",
        ShortcutId::About => "shortcut-reset-about",
        ShortcutId::Quit => "shortcut-reset-quit",
    }
}

impl LumiaApp {
    pub(crate) fn render_settings_panel(
        &self,
        window: &Window,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        self.ui.show_settings_panel.then(|| {
            let language = self.settings.language;
            let viewport_size = window.viewport_size();
            let panel_width = (f32::from(viewport_size.width) - 48.0)
                .max(320.0)
                .min(780.0);
            let panel_height = (f32::from(viewport_size.height) - 48.0)
                .max(360.0)
                .min(520.0);

            let is_recording = self.ui.recording_shortcut.is_some();

            let overlay = div()
                .id("settings-overlay")
                .absolute()
                .left(px(0.0))
                .top(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::black().opacity(0.48));

            let overlay = if is_recording {
                overlay.capture_key_down(cx.listener(Self::handle_shortcut_recording))
            } else {
                overlay
            };

            overlay.child(
                div()
                    .id("settings-panel")
                    .w(px(panel_width))
                    .h(px(panel_height))
                    .overflow_hidden()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(palette.border))
                    .bg(rgb(palette.panel_bg))
                    .shadow_lg()
                    .text_color(rgb(palette.text))
                    .flex()
                    .flex_col()
                    .on_action(cx.listener(Self::apply_selected_language))
                    .on_action(cx.listener(Self::apply_selected_theme_accent))
                    .on_action(cx.listener(Self::handle_check_for_updates))
                    .child(
                        div()
                            .id("settings-header")
                            .h(px(56.0))
                            .flex_none()
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_4()
                            .border_b_1()
                            .border_color(rgb(palette.border))
                            .child(
                                div()
                                    .flex_1()
                                    .text_sm()
                                    .child(tr(language, TextKey::SettingsTitle)),
                            )
                            .child(
                                div()
                                    .id("settings-close-button")
                                    .w(px(28.0))
                                    .h(px(28.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .hover(|style| style.bg(rgb(palette.button_hover)))
                                    .text_color(rgb(palette.text))
                                    .text_lg()
                                    .child("\u{2715}")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.close_settings_panel(cx);
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .id("settings-body")
                            .flex_1()
                            .flex()
                            .overflow_hidden()
                            .child(self.render_settings_sidebar(palette, cx))
                            .child(self.render_settings_content(window, palette, cx)),
                    ),
            )
        })
    }

    pub(crate) fn render_settings_sidebar(
        &self,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let language = self.settings.language;

        let sidebar = div()
            .id("settings-sidebar")
            .w(px(188.0))
            .h_full()
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .border_r_1()
            .border_color(rgb(palette.border))
            .bg(rgb(palette.sidebar_bg))
            .child(settings_group_button(
                "settings-group-general",
                tr(language, TextKey::General),
                self.ui.active_settings_group == SettingsGroup::General,
                palette,
                cx,
                |this, _, _, cx| {
                    this.select_settings_group(SettingsGroup::General, cx);
                },
            ));
        let sidebar = sidebar.child(settings_group_button(
            "settings-group-plugins",
            tr(language, TextKey::Plugins),
            self.ui.active_settings_group == SettingsGroup::Plugins,
            palette,
            cx,
            |this, _, _, cx| {
                this.select_settings_group(SettingsGroup::Plugins, cx);
            },
        ));
        let sidebar = sidebar.child(settings_group_button(
            "settings-group-file-associations",
            tr(language, TextKey::FileAssociations),
            self.ui.active_settings_group == SettingsGroup::FileAssociations,
            palette,
            cx,
            |this, _, _, cx| {
                this.select_settings_group(SettingsGroup::FileAssociations, cx);
            },
        ));
        sidebar
            .child(settings_group_button(
                "settings-group-shortcuts",
                tr(language, TextKey::Shortcuts),
                self.ui.active_settings_group == SettingsGroup::Shortcuts,
                palette,
                cx,
                |this, _, _, cx| {
                    this.select_settings_group(SettingsGroup::Shortcuts, cx);
                },
            ))
            .child(settings_group_button(
                "settings-group-about",
                tr(language, TextKey::About),
                self.ui.active_settings_group == SettingsGroup::About,
                palette,
                cx,
                |this, _, _, cx| {
                    this.select_settings_group(SettingsGroup::About, cx);
                },
            ))
    }

    pub(crate) fn render_settings_content(
        &self,
        window: &Window,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        match self.ui.active_settings_group {
            SettingsGroup::General => self.render_general_settings(window, cx).into_any_element(),
            SettingsGroup::Plugins => self.render_plugin_settings(palette, cx).into_any_element(),
            SettingsGroup::FileAssociations => self
                .render_file_association_settings(palette, cx)
                .into_any_element(),
            SettingsGroup::Shortcuts => self
                .render_shortcuts_settings(palette, cx)
                .into_any_element(),
            SettingsGroup::About => self.render_about_settings(palette, cx).into_any_element(),
        }
    }
}
