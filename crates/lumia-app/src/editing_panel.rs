use gpui::{
    div, px, rgb, AnyElement, Context, InteractiveElement, IntoElement, ParentElement, Styled,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::input::Input;
use gpui_component::switch::Switch;
use gpui_component::{Disableable as _, IconName};

use crate::app::LumiaApp;
use crate::editing::{CropAspect, EditMode};
use crate::i18n::{tr, TextKey};
use crate::palette::Palette;
use crate::widgets::{edit_option_button, settings_action_button};
use crate::{EDIT_PANEL_WIDTH, STATUS_BAR_HEIGHT};

impl LumiaApp {
    pub(crate) fn render_edit_panel(
        &self,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let mode = self.editing.mode?;
        let language = self.settings.language;
        let title = match mode {
            EditMode::Crop => tr(language, TextKey::EditCrop),
            EditMode::Resize => tr(language, TextKey::EditResize),
        };
        let self_handle = self.self_handle.clone();
        let close_button = Button::new("edit-panel-close")
            .ghost()
            .icon(IconName::Close)
            .disabled(self.editing.exporting)
            .on_click(move |_, _, cx| {
                let _ = self_handle.update(cx, |this, cx| {
                    this.close_edit_session(true, cx);
                });
            });

        let content = match mode {
            EditMode::Crop => self.render_crop_controls(palette, cx),
            EditMode::Resize => self.render_resize_controls(palette, cx),
        };

        Some(
            div()
                .id("edit-panel")
                .w(px(EDIT_PANEL_WIDTH))
                .h_full()
                .pb(px(STATUS_BAR_HEIGHT))
                .flex_none()
                .flex()
                .flex_col()
                .border_l_1()
                .border_color(rgb(palette.border))
                .bg(rgb(palette.panel_bg))
                .text_color(rgb(palette.text))
                .child(
                    div()
                        .h(px(52.0))
                        .flex_none()
                        .px_4()
                        .flex()
                        .items_center()
                        .border_b_1()
                        .border_color(rgb(palette.border))
                        .child(div().flex_1().text_sm().child(title))
                        .child(close_button),
                )
                .child(content)
                .child(self.render_edit_footer(palette, cx))
                .into_any_element(),
        )
    }

    fn render_crop_controls(&self, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        let language = self.settings.language;
        let rect = self.editing.crop_rect;
        div()
            .flex_1()
            .overflow_hidden()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .child(self.render_original_size(palette))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(palette.muted_text))
                            .child(tr(language, TextKey::EditAspect)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap_2()
                            .child(self.crop_aspect_button(
                                "crop-aspect-free",
                                tr(language, TextKey::EditFree),
                                CropAspect::Free,
                                palette,
                                cx,
                            ))
                            .child(self.crop_aspect_button(
                                "crop-aspect-original",
                                tr(language, TextKey::EditOriginal),
                                CropAspect::Original,
                                palette,
                                cx,
                            ))
                            .child(self.crop_aspect_button(
                                "crop-aspect-square",
                                "1:1",
                                CropAspect::Square,
                                palette,
                                cx,
                            ))
                            .child(self.crop_aspect_button(
                                "crop-aspect-four-three",
                                "4:3",
                                CropAspect::FourThree,
                                palette,
                                cx,
                            ))
                            .child(self.crop_aspect_button(
                                "crop-aspect-sixteen-nine",
                                "16:9",
                                CropAspect::SixteenNine,
                                palette,
                                cx,
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(palette.muted_text))
                                    .child(tr(language, TextKey::EditTargetSize)),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .child(format!("{} × {} px", rect.width, rect.height)),
                            ),
                    )
                    .child({
                        let handle = self.self_handle.clone();
                        settings_action_button(
                            "crop-reset",
                            tr(language, TextKey::EditReset),
                            false,
                            false,
                            move |_, _, cx| {
                                let _ = handle.update(cx, |this, cx| this.reset_crop(cx));
                            },
                        )
                    }),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(palette.muted_text))
                    .child(tr(language, TextKey::EditCropHint)),
            )
            .into_any_element()
    }

    fn render_resize_controls(&self, palette: Palette, _cx: &mut Context<Self>) -> AnyElement {
        let language = self.settings.language;
        let width_input = self.editing.width_input.as_ref().cloned();
        let height_input = self.editing.height_input.as_ref().cloned();
        let handle = self.self_handle.clone();
        let lock = Switch::new("resize-aspect-lock")
            .checked(self.editing.lock_aspect)
            .label(tr(language, TextKey::EditLockAspect))
            .on_click(move |_, window, cx| {
                let _ = handle.update(cx, |this, cx| {
                    this.toggle_resize_aspect_lock(window, cx);
                });
            });
        div()
            .flex_1()
            .overflow_hidden()
            .p_4()
            .flex()
            .flex_col()
            .gap_4()
            .child(self.render_original_size(palette))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(palette.muted_text))
                            .child(tr(language, TextKey::EditTargetSize)),
                    )
                    .children(width_input.map(|input| {
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().w(px(52.0)).text_xs().child("W"))
                            .child(div().flex_1().child(Input::new(&input)))
                            .child(div().text_xs().child("px"))
                    }))
                    .children(height_input.map(|input| {
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().w(px(52.0)).text_xs().child("H"))
                            .child(div().flex_1().child(Input::new(&input)))
                            .child(div().text_xs().child("px"))
                    }))
                    .child(lock),
            )
            .child({
                let handle = self.self_handle.clone();
                div().flex().justify_end().child(settings_action_button(
                    "resize-reset",
                    tr(language, TextKey::EditReset),
                    false,
                    self.editing.exporting,
                    move |_, window, cx| {
                        let _ = handle.update(cx, |this, cx| {
                            this.reset_resize(window, cx);
                        });
                    },
                ))
            })
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(palette.muted_text))
                    .child(tr(language, TextKey::EditResizeHint)),
            )
            .into_any_element()
    }

    fn render_original_size(&self, palette: Palette) -> impl IntoElement {
        div()
            .p_3()
            .rounded_md()
            .bg(rgb(palette.subtle_bg))
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(palette.muted_text))
                    .child(tr(self.settings.language, TextKey::EditOriginalSize)),
            )
            .child(div().text_sm().child(format!(
                "{} × {} px",
                self.editing.source_width, self.editing.source_height
            )))
    }

    fn crop_aspect_button(
        &self,
        id: &'static str,
        label: &'static str,
        aspect: CropAspect,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        edit_option_button(
            id,
            label,
            self.editing.crop_aspect == aspect,
            palette,
            cx,
            move |this, _, _, cx| this.select_crop_aspect(aspect, cx),
        )
    }

    fn render_edit_footer(&self, palette: Palette, cx: &mut Context<Self>) -> AnyElement {
        let language = self.settings.language;
        let valid = match self.editing.mode {
            Some(EditMode::Crop) => {
                self.editing.crop_rect.width > 0 && self.editing.crop_rect.height > 0
            }
            Some(EditMode::Resize) => self.resize_is_valid(cx),
            None => false,
        };
        let feedback = match &self.editing.feedback {
            Some(Ok(path)) => Some((
                format!("{} {}", tr(language, TextKey::EditExported), path.display()),
                palette.muted_text,
            )),
            Some(Err(message)) => Some((message.clone(), palette.error_text)),
            None if !valid && self.editing.mode == Some(EditMode::Resize) => Some((
                tr(language, TextKey::EditInvalidSize).into(),
                palette.error_text,
            )),
            None => None,
        };
        let cancel_handle = self.self_handle.clone();
        let export_handle = self.self_handle.clone();
        div()
            .flex_none()
            .p_4()
            .border_t_1()
            .border_color(rgb(palette.border))
            .flex()
            .flex_col()
            .gap_3()
            .children(feedback.map(|(message, color)| {
                div()
                    .text_xs()
                    .text_color(rgb(color))
                    .overflow_hidden()
                    .child(message)
            }))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(settings_action_button(
                        "edit-cancel",
                        tr(language, TextKey::Cancel),
                        false,
                        self.editing.exporting,
                        move |_, _, cx| {
                            let _ = cancel_handle.update(cx, |this, cx| {
                                this.close_edit_session(true, cx);
                            });
                        },
                    ))
                    .child(settings_action_button(
                        "edit-export",
                        if self.editing.exporting {
                            tr(language, TextKey::EditExporting)
                        } else {
                            tr(language, TextKey::EditExportCopy)
                        },
                        true,
                        !valid || self.editing.exporting,
                        move |_, _, cx| {
                            let _ = export_handle.update(cx, |this, cx| this.export_edit_copy(cx));
                        },
                    )),
            )
            .into_any_element()
    }
}
