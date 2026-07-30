use gpui::{
    div, Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, Window,
};
use gpui_component::switch::Switch;
use lumia_core::{Language, ThemeAccent, ThemeMode};

use crate::app::LumiaApp;
use crate::i18n::{tr, TextKey};
use crate::settings_ui::{language_text_key, theme_accent_text_key, theme_mode_text_key};
use crate::widgets::{settings_dropdown_button, settings_label};
use crate::{SelectLanguage, SelectThemeAccent, SelectThemeMode};

impl LumiaApp {
    pub(crate) fn render_general_settings(
        &self,
        _window: &Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let language = self.settings.language;
        let selected_language = self.settings.language;
        let selected_theme = self.settings.theme;
        let selected_accent = self.settings.theme_accent;
        let check_updates_on_startup = self.settings.check_updates_on_startup;
        let self_handle = self.self_handle.clone();

        div()
            .id("settings-general")
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .gap_5()
            .p_5()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .child(settings_label(tr(language, TextKey::Language)))
                    .child(settings_dropdown_button(
                        "settings-language-select",
                        tr(language, language_text_key(selected_language)),
                        move |menu, _, _| {
                            menu.menu_with_check(
                                    tr(language, TextKey::English),
                                    selected_language == Language::English,
                                    Box::new(SelectLanguage(Language::English)),
                                )
                                .menu_with_check(
                                    tr(language, TextKey::Chinese),
                                    selected_language == Language::Chinese,
                                    Box::new(SelectLanguage(Language::Chinese)),
                                )
                        },
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .child(settings_label(tr(language, TextKey::Theme)))
                    .child(settings_dropdown_button(
                        "settings-theme-select",
                        tr(language, theme_mode_text_key(selected_theme)),
                        move |menu, _, _| {
                            menu.menu_with_check(
                                    tr(language, TextKey::Light),
                                    selected_theme == ThemeMode::Light,
                                    Box::new(SelectThemeMode(ThemeMode::Light)),
                                )
                                .menu_with_check(
                                    tr(language, TextKey::Dark),
                                    selected_theme == ThemeMode::Dark,
                                    Box::new(SelectThemeMode(ThemeMode::Dark)),
                                )
                                .menu_with_check(
                                    tr(language, TextKey::FollowSystem),
                                    selected_theme == ThemeMode::FollowSystem,
                                    Box::new(SelectThemeMode(ThemeMode::FollowSystem)),
                                )
                        },
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .child(settings_label(tr(language, TextKey::ThemeColor)))
                    .child(settings_dropdown_button(
                        "settings-theme-accent-select",
                        tr(language, theme_accent_text_key(selected_accent)),
                        move |menu, _, _| {
                            menu.menu_with_check(
                                    tr(language, TextKey::AccentBlue),
                                    selected_accent == ThemeAccent::Blue,
                                    Box::new(SelectThemeAccent(ThemeAccent::Blue)),
                                )
                                .menu_with_check(
                                    tr(language, TextKey::AccentGreen),
                                    selected_accent == ThemeAccent::Green,
                                    Box::new(SelectThemeAccent(ThemeAccent::Green)),
                                )
                                .menu_with_check(
                                    tr(language, TextKey::AccentOrange),
                                    selected_accent == ThemeAccent::Orange,
                                    Box::new(SelectThemeAccent(ThemeAccent::Orange)),
                                )
                                .menu_with_check(
                                    tr(language, TextKey::AccentRose),
                                    selected_accent == ThemeAccent::Rose,
                                    Box::new(SelectThemeAccent(ThemeAccent::Rose)),
                                )
                        },
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .child(settings_label(tr(language, TextKey::CheckUpdatesOnStartup)))
                    .child(
                        Switch::new("settings-check-updates-startup")
                            .checked(check_updates_on_startup)
                            .on_click({
                                let self_handle = self_handle.clone();
                                move |checked, _, cx| {
                                    let _ = self_handle.update(cx, |this, cx| {
                                        this.set_check_updates_on_startup(*checked, cx);
                                    });
                                }
                            }),
                    ),
            )
    }
}
