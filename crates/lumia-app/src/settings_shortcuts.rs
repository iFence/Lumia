use gpui::{
    div, px, rgb, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled,
};
use lumia_core::ShortcutId;

use crate::app::LumiaApp;
use crate::i18n::{tr, TextKey};
use crate::palette::Palette;
use crate::settings_ui::{shortcut_record_button_id, shortcut_reset_button_id};
use crate::widgets::{settings_action_button, shortcut_record_button, shortcut_reset_button};

impl LumiaApp {
    pub(crate) fn render_shortcuts_settings(
        &self,
        palette: Palette,
        cx: &mut Context<LumiaApp>,
    ) -> impl IntoElement {
        let language = self.settings.language;

        let shortcut_rows: Vec<(ShortcutId, TextKey)> = vec![
            (ShortcutId::OpenFile, TextKey::ShortcutOpenFileLabel),
            (ShortcutId::ZoomIn, TextKey::ShortcutZoomInLabel),
            (ShortcutId::ZoomOut, TextKey::ShortcutZoomOutLabel),
            (ShortcutId::ZoomFit, TextKey::ShortcutZoomFitLabel),
            (
                ShortcutId::ToggleFullscreen,
                TextKey::ShortcutToggleFullscreenLabel,
            ),
            (
                ShortcutId::ExitFullscreen,
                TextKey::ShortcutExitFullscreenLabel,
            ),
            (
                ShortcutId::ToggleImageInfo,
                TextKey::ShortcutToggleImageInfoLabel,
            ),
            (ShortcutId::NextImage, TextKey::ShortcutNextImageLabel),
            (
                ShortcutId::PreviousImage,
                TextKey::ShortcutPreviousImageLabel,
            ),
            (
                ShortcutId::OpenSettings,
                TextKey::ShortcutOpenSettingsLabel,
            ),
            (ShortcutId::About, TextKey::ShortcutAboutLabel),
            (ShortcutId::Quit, TextKey::ShortcutQuitLabel),
        ];
        let shortcut_row_count = shortcut_rows.len();

        div()
            .id("settings-shortcuts")
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .gap_4()
            .p_5()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .child(tr(language, TextKey::Shortcuts)),
                    )
                    .child(settings_action_button(
                        "shortcuts-reset-all",
                        tr(language, TextKey::ShortcutResetAll),
                        false,
                        false,
                        {
                            let self_handle = self.self_handle.clone();
                            move |_, _, cx| {
                                let _ = self_handle.update(cx, |this, cx| {
                                    this.reset_all_shortcuts(cx);
                                });
                            }
                        },
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(palette.border))
                    .bg(rgb(palette.panel_bg))
                    .children(shortcut_rows.into_iter().enumerate().map(
                        |(index, (shortcut_id, label))| {
                            let is_recording = self.ui.recording_shortcut == Some(shortcut_id);
                            let current_binding = self.get_shortcut_binding(shortcut_id);
                            let label_text = tr(language, label);
                            let separator = if index + 1 == shortcut_row_count {
                                rgb(palette.border).opacity(0.0)
                            } else {
                                rgb(palette.border)
                            };

                            div()
                                .h(px(48.0))
                                .flex()
                                .items_center()
                                .justify_between()
                                .gap_3()
                                .px_3()
                                .border_b_1()
                                .border_color(separator)
                                .hover(move |style| style.bg(rgb(palette.subtle_bg)))
                                .child(div().flex_1().text_sm().child(label_text))
                                .child(shortcut_record_button(
                                    shortcut_record_button_id(shortcut_id),
                                    current_binding,
                                    is_recording,
                                    cx,
                                    move |this, _, _, cx| {
                                        if this.ui.recording_shortcut == Some(shortcut_id) {
                                            this.stop_recording_shortcut(cx);
                                        } else {
                                            this.start_recording_shortcut(shortcut_id, cx);
                                        }
                                    },
                                ))
                                .child(shortcut_reset_button(
                                    shortcut_reset_button_id(shortcut_id),
                                    tr(language, TextKey::ShortcutResetToDefault),
                                    cx,
                                    move |this, _, _, cx| {
                                        this.reset_shortcut(shortcut_id, cx);
                                    },
                                ))
                        },
                    )),
            )
    }
}
