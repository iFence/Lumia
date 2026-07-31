use gpui::{
    div, px, rgb, ClipboardItem, Context, InteractiveElement, IntoElement, MouseButton,
    ParentElement, StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{tooltip::Tooltip, Icon};
use lumia_core::{ExifMetadata, Language};

use crate::app::LumiaApp;
use crate::i18n::{tr, TextKey};
use crate::util::{format_file_size, format_modified_time};

impl LumiaApp {
    pub(crate) fn render_image_info_overlay(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        (self.ui.show_image_info && self.image_path().is_some() && !self.loads.is_transitioning())
            .then(|| {
                let language = self.settings.language;
                let tooltip = tr(language, TextKey::CopyImageInfo);
                let close_tooltip = tr(language, TextKey::CloseImageInfo);
                div()
                    .id("image-info-overlay")
                    .absolute()
                    .top_4()
                    .left_4()
                    .max_w(px(560.0))
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(gpui::black().opacity(0.72))
                    .text_color(rgb(0xf2f2f2))
                    .text_xs()
                    .shadow_md()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_end()
                            .gap_1()
                            .child(
                                div()
                                    .id("image-info-copy")
                                    .w(px(24.0))
                                    .h(px(20.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .hover(move |style| {
                                        style.bg(gpui::white().opacity(0.12))
                                    })
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(tooltip).build(window, cx)
                                    })
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, window, cx| {
                                            let text = this
                                                .image_info_lines(this.settings.language, window)
                                                .join("\n");
                                            cx.write_to_clipboard(ClipboardItem::new_string(text));
                                        }),
                                    )
                                    .child(
                                        Icon::default()
                                            .path("custom/copy.svg")
                                            .size(px(14.0))
                                            .text_color(rgb(0xf2f2f2)),
                                    ),
                            )
                            .child(
                                div()
                                    .id("image-info-close")
                                    .w(px(24.0))
                                    .h(px(20.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_sm()
                                    .cursor_pointer()
                                    .hover(move |style| {
                                        style.bg(gpui::white().opacity(0.12))
                                    })
                                    .tooltip(move |window, cx| {
                                        Tooltip::new(close_tooltip).build(window, cx)
                                    })
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|this, _, _, cx| {
                                            this.ui.show_image_info = false;
                                            cx.notify();
                                        }),
                                    )
                                    .child(
                                        Icon::default()
                                            .path("custom/close.svg")
                                            .size(px(14.0))
                                            .text_color(rgb(0xf2f2f2)),
                                    ),
                            ),
                    )
                    .children(
                        self.image_info_lines(language, window)
                            .into_iter()
                            .map(|line| div().child(line)),
                    )
            })
    }

    pub(crate) fn image_info_lines(&self, language: Language, window: &Window) -> Vec<String> {
        let Some(path) = self.image_path() else {
            return Vec::new();
        };

        let unknown = tr(language, TextKey::ImageInfoUnknown);
        let mut lines = Vec::new();

        lines.push(format!(
            "{}: {}",
            tr(language, TextKey::ImageInfoName),
            self.image_name()
        ));

        if let Some(file_metadata) = self.loads.file_metadata() {
            lines.push(format!(
                "{}: {}",
                tr(language, TextKey::ImageInfoFileSize),
                format_file_size(file_metadata.size_bytes)
            ));
            if let Some(modified) = file_metadata.modified {
                lines.push(format!(
                    "{}: {}",
                    tr(language, TextKey::ImageInfoModified),
                    format_modified_time(modified)
                ));
            }
        }

        if let Some(metadata) = self
            .viewer
            .document()
            .and_then(|image| image.metadata.as_ref())
        {
            lines.push(format!(
                "{}: {} × {}",
                tr(language, TextKey::ImageInfoDimensions),
                metadata.width,
                metadata.height
            ));
            let format_details = [
                metadata.format_name.as_deref(),
                metadata.exif.chroma_subsampling.as_deref(),
                metadata.exif.color_space.as_deref(),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
            if !format_details.is_empty() {
                lines.push(format!(
                    "{}: {}",
                    tr(language, TextKey::ImageInfoFormat),
                    format_details.join(", ")
                ));
            }
            append_exif_lines(&mut lines, language, &metadata.exif);
        } else {
            lines.push(format!(
                "{}: {unknown}",
                tr(language, TextKey::ImageInfoDimensions),
            ));
        }

        lines.push(format!(
            "{}: {:.0}%",
            tr(language, TextKey::ImageInfoZoom),
            self.image_display_scale(window).unwrap_or(1.0) * 100.0
        ));
        lines.push(format!(
            "{}: {}",
            tr(language, TextKey::ImageInfoPath),
            path.display()
        ));
        lines
    }
}

fn append_exif_lines(lines: &mut Vec<String>, language: Language, exif: &ExifMetadata) {
    for (key, value) in [
        (TextKey::ImageInfoCameraMake, exif.camera_make.as_deref()),
        (TextKey::ImageInfoCameraModel, exif.camera_model.as_deref()),
        (TextKey::ImageInfoLens, exif.lens.as_deref()),
        (TextKey::ImageInfoSoftware, exif.software.as_deref()),
        (TextKey::ImageInfoDateTaken, exif.date_taken.as_deref()),
        (TextKey::ImageInfoFlash, exif.flash.as_deref()),
        (TextKey::ImageInfoFocalLength, exif.focal_length.as_deref()),
        (
            TextKey::ImageInfoExposureTime,
            exif.exposure_time.as_deref(),
        ),
        (
            TextKey::ImageInfoExposureBias,
            exif.exposure_bias.as_deref(),
        ),
        (TextKey::ImageInfoAperture, exif.aperture.as_deref()),
        (TextKey::ImageInfoIso, exif.iso.as_deref()),
        (
            TextKey::ImageInfoExposureProgram,
            exif.exposure_program.as_deref(),
        ),
        (
            TextKey::ImageInfoMeteringMode,
            exif.metering_mode.as_deref(),
        ),
        (TextKey::ImageInfoGps, exif.gps.as_deref()),
    ] {
        if let Some(value) = value {
            lines.push(format!("{}: {value}", tr(language, key)));
        }
    }
}
