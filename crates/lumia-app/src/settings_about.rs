use gpui::prelude::FluentBuilder;
use gpui::{
    div, px, rgb, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled,
};

use crate::app::LumiaApp;
use crate::i18n::{tr, TextKey};
use crate::palette::Palette;
use crate::update_check::UpdateState;
use crate::widgets::settings_action_button;
use crate::APP_TITLE;

impl LumiaApp {
    pub(crate) fn render_about_settings(
        &self,
        palette: Palette,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let language = self.settings.language;
        let update_state = &self.ui.update_check.state;
        let is_busy = self.ui.update_check.is_busy();
        let has_update = self.ui.update_check.has_update();

        let status_element: gpui::AnyElement = match update_state {
            UpdateState::Idle => div().into_any_element(),
            UpdateState::Checking => div()
                .text_sm()
                .text_color(rgb(palette.muted_text))
                .child(tr(language, TextKey::CheckingForUpdates))
                .into_any_element(),
            UpdateState::Available { latest_version, .. } => div()
                .text_sm()
                .font_weight(FontWeight::MEDIUM)
                .text_color(rgb(palette.accent))
                .child(format!(
                    "{} v{}",
                    tr(language, TextKey::UpdateAvailable),
                    latest_version
                ))
                .into_any_element(),
            UpdateState::UpToDate => div()
                .text_sm()
                .text_color(rgb(palette.muted_text))
                .child(tr(language, TextKey::UpToDate))
                .into_any_element(),
            UpdateState::Error(message) => div()
                .text_sm()
                .text_color(rgb(palette.error_text))
                .child(format!(
                    "{}: {message}",
                    tr(language, TextKey::UpdateCheckFailed)
                ))
                .into_any_element(),
        };

        let check_button_handle = self.self_handle.clone();
        let open_button_handle = self.self_handle.clone();

        let release_notes_element =
            if let UpdateState::Available { release_notes, .. } = update_state {
                if release_notes.trim().is_empty() {
                    None
                } else {
                    Some(
                        div()
                            .id("about-release-notes")
                            .w_full()
                            .max_h(px(220.0))
                            .overflow_y_scroll()
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(palette.border))
                            .bg(rgb(palette.sidebar_bg))
                            .p_3()
                            .text_xs()
                            .text_color(rgb(palette.muted_text))
                            .child(release_notes.clone()),
                    )
                }
            } else {
                None
            };

        div()
            .id("settings-about")
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .gap_3()
            .p_5()
            .overflow_y_scroll()
            .child(
                div()
                    .text_xl()
                    .font_weight(FontWeight::BOLD)
                    .text_color(rgb(palette.text))
                    .child(APP_TITLE),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(palette.muted_text))
                    .child(format!(
                        "{} {}",
                        tr(language, TextKey::Version),
                        env!("CARGO_PKG_VERSION")
                    )),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(settings_action_button(
                        "about-check-updates",
                        tr(language, TextKey::CheckForUpdates),
                        false,
                        is_busy,
                        move |_, _, cx| {
                            let _ = check_button_handle.update(cx, |this, cx| {
                                this.check_for_updates(true, cx);
                            });
                        },
                    ))
                    .child(status_element),
            )
            .when_some(release_notes_element, |this, notes| {
                this.child(
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(rgb(palette.text))
                                .child(tr(language, TextKey::ReleaseNotes)),
                        )
                        .child(notes),
                )
            })
            .when(has_update, |this| {
                this.child(settings_action_button(
                    "about-open-releases",
                    tr(language, TextKey::OpenReleasesPage),
                    true,
                    false,
                    move |_, _, cx| {
                        let _ = open_button_handle.update(cx, |this, cx| {
                            this.open_releases_page(cx);
                        });
                    },
                ))
            })
    }
}
