use gpui::{div, px, rgb, Context, InteractiveElement, IntoElement, ParentElement, Styled, Window};
use lumia_core::{Language, SettingsGroup, ThemeMode};

use crate::app::LumiaApp;
use crate::i18n::{tr, TextKey};
use crate::palette::{theme_resolves_to_dark, Palette};
use crate::widgets::{
    settings_group_button, settings_label, settings_option_button, settings_section_title,
    toolbar_button,
};

impl LumiaApp {
    pub(crate) fn render_settings_panel(
        &self,
        window: &Window,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        self.show_settings_panel.then(|| {
            let language = self.settings.language;
            let viewport_size = window.viewport_size();
            let panel_width = (f32::from(viewport_size.width) - 48.0)
                .max(320.0)
                .min(780.0);
            let panel_height = (f32::from(viewport_size.height) - 48.0)
                .max(360.0)
                .min(520.0);

            div()
                .id("settings-overlay")
                .absolute()
                .left(px(0.0))
                .top(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::black().opacity(0.48))
                .child(
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
                        .child(
                            div()
                                .id("settings-header")
                                .h(px(56.0))
                                .flex()
                                .items_center()
                                .gap_2()
                                .px_4()
                                .border_b_1()
                                .border_color(rgb(palette.border))
                                .child(
                                    div()
                                        .flex_1()
                                        .flex()
                                        .flex_col()
                                        .child(
                                            div()
                                                .text_sm()
                                                .child(tr(language, TextKey::SettingsTitle)),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(palette.muted_text))
                                                .child(tr(language, TextKey::SettingsDescription)),
                                        ),
                                )
                                .child(toolbar_button(
                                    "settings-close-button",
                                    tr(language, TextKey::Close),
                                    palette,
                                    cx,
                                    |this, _, _, cx| {
                                        this.close_settings_panel(cx);
                                    },
                                )),
                        )
                        .child(
                            div()
                                .id("settings-body")
                                .flex_1()
                                .flex()
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

        div()
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
                self.active_settings_group == SettingsGroup::General,
                palette,
                cx,
                |this, _, _, cx| {
                    this.select_settings_group(SettingsGroup::General, cx);
                },
            ))
            .child(settings_group_button(
                "settings-group-shortcuts",
                tr(language, TextKey::Shortcuts),
                self.active_settings_group == SettingsGroup::Shortcuts,
                palette,
                cx,
                |this, _, _, cx| {
                    this.select_settings_group(SettingsGroup::Shortcuts, cx);
                },
            ))
    }

    pub(crate) fn render_settings_content(
        &self,
        window: &Window,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        match self.active_settings_group {
            SettingsGroup::General => self
                .render_general_settings(window, palette, cx)
                .into_any_element(),
            SettingsGroup::Shortcuts => self.render_shortcuts_settings(palette).into_any_element(),
        }
    }

    pub(crate) fn render_general_settings(
        &self,
        window: &Window,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let language = self.settings.language;
        let resolved_theme = if theme_resolves_to_dark(self.settings.theme, window.appearance()) {
            tr(language, TextKey::Dark)
        } else {
            tr(language, TextKey::Light)
        };

        div()
            .id("settings-general")
            .flex_1()
            .flex()
            .flex_col()
            .gap_5()
            .p_5()
            .child(settings_section_title(
                tr(language, TextKey::General),
                tr(language, TextKey::GeneralDescription),
                palette,
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .child(settings_label(
                        tr(language, TextKey::Language),
                        tr(language, TextKey::LanguageDescription),
                        palette,
                    ))
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(settings_option_button(
                                "language-english",
                                tr(language, TextKey::English),
                                self.settings.language == Language::English,
                                palette,
                                cx,
                                |this, _, _, cx| {
                                    this.set_language(Language::English, cx);
                                },
                            ))
                            .child(settings_option_button(
                                "language-chinese",
                                tr(language, TextKey::Chinese),
                                self.settings.language == Language::Chinese,
                                palette,
                                cx,
                                |this, _, _, cx| {
                                    this.set_language(Language::Chinese, cx);
                                },
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .child(settings_label(
                        tr(language, TextKey::Theme),
                        tr(language, TextKey::ThemeDescription),
                        palette,
                    ))
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(settings_option_button(
                                "theme-light",
                                tr(language, TextKey::Light),
                                self.settings.theme == ThemeMode::Light,
                                palette,
                                cx,
                                |this, _, _, cx| {
                                    this.set_theme(ThemeMode::Light, cx);
                                },
                            ))
                            .child(settings_option_button(
                                "theme-dark",
                                tr(language, TextKey::Dark),
                                self.settings.theme == ThemeMode::Dark,
                                palette,
                                cx,
                                |this, _, _, cx| {
                                    this.set_theme(ThemeMode::Dark, cx);
                                },
                            ))
                            .child(settings_option_button(
                                "theme-system",
                                tr(language, TextKey::FollowSystem),
                                self.settings.theme == ThemeMode::FollowSystem,
                                palette,
                                cx,
                                |this, _, _, cx| {
                                    this.set_theme(ThemeMode::FollowSystem, cx);
                                },
                            )),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(palette.muted_text))
                    .child(format!(
                        "{}: {resolved_theme}",
                        tr(language, TextKey::ResolvedTheme)
                    )),
            )
    }

    pub(crate) fn render_shortcuts_settings(&self, palette: Palette) -> impl IntoElement {
        let language = self.settings.language;
        let shortcuts = [
            (TextKey::ShortcutOpenFile, "Ctrl+O / Cmd+O"),
            (TextKey::ShortcutZoomIn, "Ctrl++ / Cmd++"),
            (TextKey::ShortcutZoomOut, "Ctrl+- / Cmd+-"),
            (TextKey::ShortcutZoomFit, "Ctrl+0 / Cmd+0"),
            (TextKey::ShortcutFullscreen, "F11 / Ctrl+Enter / Cmd+Enter"),
            (TextKey::ShortcutImageInfo, "Tab"),
            (TextKey::ShortcutQuit, "Ctrl+Q / Cmd+Q"),
        ];

        div()
            .id("settings-shortcuts")
            .flex_1()
            .flex()
            .flex_col()
            .gap_4()
            .p_5()
            .child(settings_section_title(
                tr(language, TextKey::Shortcuts),
                tr(language, TextKey::ShortcutsDescription),
                palette,
            ))
            .children(shortcuts.into_iter().map(|(label, binding)| {
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(palette.subtle_bg))
                    .child(div().text_sm().child(tr(language, label)))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(palette.muted_text))
                            .child(binding),
                    )
            }))
    }
}
