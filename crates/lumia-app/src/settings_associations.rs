use gpui::{
    div, px, rgb, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled,
};
use gpui_component::{Icon, IconName};
use lumia_core::SUPPORTED_IMAGE_EXTENSIONS;

use crate::app::LumiaApp;
use crate::file_association_state::FileAssociationFeedback;
use crate::i18n::{tr, TextKey};
use crate::palette::Palette;
use crate::settings_association_formats::FILE_ASSOCIATION_CATEGORIES;
use crate::widgets::settings_action_button;

impl LumiaApp {
    pub(crate) fn render_file_association_settings(
        &self,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let language = self.settings.language;
        let selected = &self.ui.file_associations.selected_extensions;
        let all_selected = selected.len() == SUPPORTED_IMAGE_EXTENSIONS.len();
        let has_selection = !selected.is_empty();
        let is_dirty = self.ui.file_associations.is_dirty();
        let is_busy = self.ui.file_associations.is_busy;
        let selected_summary = format!(
            "{} {}/{}",
            tr(language, TextKey::SelectedFormats),
            selected.len(),
            SUPPORTED_IMAGE_EXTENSIONS.len()
        );

        let header = div()
            .flex()
            .items_start()
            .justify_between()
            .gap_4()
            .child(
                div().flex_1().flex().flex_col().gap_1().child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .child(tr(language, TextKey::FileAssociations)),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(palette.muted_text))
                                .child(selected_summary),
                        ),
                ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(settings_action_button(
                        "file-associations-select-all",
                        tr(language, TextKey::SelectAll),
                        false,
                        is_busy || all_selected,
                        {
                            let self_handle = self.self_handle.clone();
                            move |_, _, cx| {
                                let _ = self_handle.update(cx, |this, cx| {
                                    this.select_all_file_associations(cx);
                                });
                            }
                        },
                    ))
                    .child(settings_action_button(
                        "file-associations-clear",
                        tr(language, TextKey::ClearAll),
                        false,
                        is_busy || !has_selection,
                        {
                            let self_handle = self.self_handle.clone();
                            move |_, _, cx| {
                                let _ = self_handle.update(cx, |this, cx| {
                                    this.clear_file_associations(cx);
                                });
                            }
                        },
                    )),
            );

        let mut formats = div()
            .id("file-association-format-grid")
            .flex_1()
            .flex()
            .flex_col()
            .gap_4()
            .content_start()
            .overflow_y_scroll()
            .pr_1();
        for category in FILE_ASSOCIATION_CATEGORIES {
            let category_selected = category
                .extensions
                .iter()
                .filter(|extension| selected.contains(**extension))
                .count();
            let category_count = format!("{category_selected}/{}", category.extensions.len());
            let mut grid = div().grid().grid_cols(5).gap_2();

            for &extension in category.extensions {
                let checked = selected.contains(extension);
                let self_handle = self.self_handle.clone();
                grid = grid.child(
                    div()
                        .id(format!("file-association-card-{extension}"))
                        .w_full()
                        .h(px(38.0))
                        .px_2()
                        .flex()
                        .items_center()
                        .rounded_md()
                        .hover(move |style| style.bg(rgb(palette.button_hover)))
                        .child({
                            let self_handle = self_handle.clone();
                            div()
                                .id(format!("file-association-{extension}"))
                                .flex()
                                .items_center()
                                .gap_2()
                                .cursor_pointer()
                                .on_click(move |_, _, cx| {
                                    let _ = self_handle.update(cx, |this, cx| {
                                        this.set_file_association_extension(
                                            extension, !checked, cx,
                                        );
                                    });
                                })
                                .child(
                                    div()
                                        .w(px(16.0))
                                        .h(px(16.0))
                                        .rounded_sm()
                                        .border_1()
                                        .border_color(rgb(if checked {
                                            palette.accent
                                        } else {
                                            palette.border
                                        }))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .children(checked.then(|| {
                                            div()
                                                .w(px(16.0))
                                                .h(px(16.0))
                                                .rounded_sm()
                                                .bg(rgb(palette.accent))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .child(
                                                    Icon::new(IconName::Check)
                                                        .size(px(12.0))
                                                        .text_color(rgb(0xffffff)),
                                                )
                                        })),
                                )
                                .child(div().text_sm().child(extension.to_ascii_uppercase()))
                        }),
                );
            }

            formats = formats.child(
                div()
                    .id(format!("file-association-category-{}", category.id))
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(tr(language, category.title)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(palette.muted_text))
                                    .child(category_count),
                            ),
                    )
                    .child(grid),
            );
        }

        let footer = div()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .pt_3()
            .border_t_1()
            .border_color(rgb(palette.border))
            .child(self.render_file_association_feedback(palette, cx))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(settings_action_button(
                        "file-associations-refresh",
                        tr(language, TextKey::RefreshAssociations),
                        false,
                        is_busy,
                        {
                            let self_handle = self.self_handle.clone();
                            move |_, _, cx| {
                                let _ = self_handle.update(cx, |this, cx| {
                                    this.refresh_file_associations(cx);
                                });
                            }
                        },
                    ))
                    .child(settings_action_button(
                        "file-associations-apply",
                        tr(
                            language,
                            if has_selection {
                                TextKey::ApplyAssociations
                            } else {
                                TextKey::RemoveAssociations
                            },
                        ),
                        true,
                        is_busy || !is_dirty,
                        {
                            let self_handle = self.self_handle.clone();
                            move |_, _, cx| {
                                let _ = self_handle.update(cx, |this, cx| {
                                    this.apply_selected_file_associations(cx);
                                });
                            }
                        },
                    )),
            );

        div()
            .id("settings-file-associations")
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .gap_4()
            .p_5()
            .child(header)
            .child(formats)
            .child(footer)
    }

    fn render_file_association_feedback(
        &self,
        palette: Palette,
        _cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let language = self.settings.language;
        let (message, is_error) = if self.ui.file_associations.is_busy {
            (tr(language, TextKey::AssociationLoading).to_string(), false)
        } else {
            match &self.ui.file_associations.feedback {
                Some(FileAssociationFeedback::Applied) => {
                    (tr(language, TextKey::AssociationApplied).to_string(), false)
                }
                Some(FileAssociationFeedback::Removed) => {
                    (tr(language, TextKey::AssociationRemoved).to_string(), false)
                }
                Some(FileAssociationFeedback::NeedsSystemConfirmation) => (
                    tr(language, TextKey::AssociationNeedsConfirmation).to_string(),
                    false,
                ),
                Some(FileAssociationFeedback::ManualRestore(extensions)) => (
                    format!(
                        "{}: {}",
                        tr(language, TextKey::AssociationManualRestore),
                        extensions.join(", ")
                    ),
                    true,
                ),
                Some(FileAssociationFeedback::Error(error)) => (
                    format!("{}: {error}", tr(language, TextKey::AssociationApplyFailed)),
                    true,
                ),
                Some(FileAssociationFeedback::SettingsLaunchError(error)) => (
                    format!(
                        "{}: {error}",
                        tr(language, TextKey::DefaultAppsLaunchFailed)
                    ),
                    true,
                ),
                None => return div().flex_1().into_any_element(),
            }
        };
        let feedback = div()
            .flex_1()
            .flex()
            .items_center()
            .gap_2()
            .text_xs()
            .text_color(rgb(if is_error {
                palette.error_text
            } else {
                palette.muted_text
            }))
            .child(message);
        if matches!(
            self.ui.file_associations.feedback,
            Some(FileAssociationFeedback::SettingsLaunchError(_))
        ) {
            feedback
                .child(settings_action_button(
                    "file-associations-retry-settings",
                    tr(language, TextKey::RetrySystemSettings),
                    false,
                    false,
                    {
                        let self_handle = self.self_handle.clone();
                        move |_, _, cx| {
                            let _ = self_handle.update(cx, |this, cx| {
                                this.retry_default_apps_settings(cx);
                            });
                        }
                    },
                ))
                .into_any_element()
        } else {
            feedback.into_any_element()
        }
    }
}
