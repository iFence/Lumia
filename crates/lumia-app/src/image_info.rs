use gpui::{div, px, rgb, InteractiveElement, IntoElement, ParentElement, Styled};
use std::fs;

use crate::app::LumiaApp;
use crate::i18n::{tr, TextKey};
use crate::util::{format_file_size, format_modified_time};
use lumia_core::Language;

impl LumiaApp {
    pub(crate) fn render_image_info_overlay(&self) -> Option<impl IntoElement> {
        (self.show_image_info && self.image_path().is_some()).then(|| {
            let language = self.settings.language;
            div()
                .id("image-info-overlay")
                .absolute()
                .top_4()
                .left_4()
                .max_w(px(420.0))
                .px_3()
                .py_2()
                .rounded_md()
                .bg(gpui::black().opacity(0.72))
                .text_color(rgb(0xf2f2f2))
                .text_xs()
                .shadow_md()
                .children(
                    self.image_info_lines(language)
                        .into_iter()
                        .map(|line| div().child(line)),
                )
        })
    }

    pub(crate) fn image_info_lines(&self, language: Language) -> Vec<String> {
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

        if let Some(metadata) = self
            .current_image
            .as_ref()
            .and_then(|image| image.metadata.as_ref())
        {
            lines.push(format!(
                "{}: {} × {}",
                tr(language, TextKey::ImageInfoDimensions),
                metadata.width,
                metadata.height
            ));
            if let Some(format_name) = metadata.format_name.as_deref() {
                lines.push(format!(
                    "{}: {format_name}",
                    tr(language, TextKey::ImageInfoFormat),
                ));
            }
        } else {
            lines.push(format!(
                "{}: {unknown}",
                tr(language, TextKey::ImageInfoDimensions),
            ));
        }

        if let Ok(file_metadata) = fs::metadata(path) {
            lines.push(format!(
                "{}: {}",
                tr(language, TextKey::ImageInfoFileSize),
                format_file_size(file_metadata.len())
            ));
            if let Ok(modified) = file_metadata.modified() {
                lines.push(format!(
                    "{}: {}",
                    tr(language, TextKey::ImageInfoModified),
                    format_modified_time(modified)
                ));
            }
        }

        lines.push(format!(
            "{}: {:.0}%",
            tr(language, TextKey::ImageInfoZoom),
            self.viewport.zoom * 100.0
        ));
        lines.push(format!(
            "{}: {}",
            tr(language, TextKey::ImageInfoPath),
            path.display()
        ));
        lines
    }
}
