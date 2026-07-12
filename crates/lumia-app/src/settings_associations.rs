use gpui::{
    div, rgb, Context, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled,
};
use gpui_component::checkbox::Checkbox;
use lumia_core::{supported_image_format_groups, SUPPORTED_IMAGE_EXTENSIONS};

use crate::app::LumiaApp;
use crate::file_association_state::FileAssociationFeedback;
use crate::i18n::{tr, TextKey};
use crate::palette::Palette;
use crate::widgets::{settings_action_button, settings_label};

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

        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .child(settings_label(
                tr(language, TextKey::FileAssociations),
                tr(language, TextKey::FileAssociationsDescription),
                palette,
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(settings_action_button(
                        "file-associations-select-all",
                        tr(language, TextKey::SelectAll),
                        false,
                        all_selected,
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
                        !has_selection,
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
            .id("file-association-format-list")
            .flex_1()
            .flex()
            .flex_col()
            .gap_1()
            .overflow_y_scroll();
        for group in supported_image_format_groups() {
            let label = format!(
                "{} ({})",
                group.name,
                group
                    .extensions
                    .iter()
                    .map(|extension| format!(".{extension}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            let checked = group
                .extensions
                .iter()
                .all(|extension| selected.contains(*extension));
            let extensions = group.extensions;
            let self_handle = self.self_handle.clone();
            formats = formats.child(
                div().w_full().py_1().child(
                    Checkbox::new(format!("file-association-{}", group.id))
                        .checked(checked)
                        .label(label)
                        .on_click(move |checked, _, cx| {
                            let _ = self_handle.update(cx, |this, cx| {
                                this.set_file_association_group(extensions, *checked, cx);
                            });
                        }),
                ),
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
                !is_dirty,
                {
                    let self_handle = self.self_handle.clone();
                    move |_, _, cx| {
                        let _ = self_handle.update(cx, |this, cx| {
                            this.apply_selected_file_associations(cx);
                        });
                    }
                },
            ));

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
        let (message, is_error) = match &self.ui.file_associations.feedback {
            Some(FileAssociationFeedback::Applied) => {
                (tr(language, TextKey::AssociationApplied).to_string(), false)
            }
            Some(FileAssociationFeedback::Removed) => {
                (tr(language, TextKey::AssociationRemoved).to_string(), false)
            }
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
            None => (
                tr(language, TextKey::FileAssociationsSystemNotice).to_string(),
                false,
            ),
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
