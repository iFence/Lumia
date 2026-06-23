use gpui::{div, px, rgb, InteractiveElement, IntoElement, ParentElement, Styled};
use std::fs;

use crate::app::LumiaApp;
use crate::util::{format_file_size, format_modified_time};

impl LumiaApp {
    pub(crate) fn render_image_info_overlay(&self) -> Option<impl IntoElement> {
        (self.show_image_info && self.image_path().is_some()).then(|| {
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
                    self.image_info_lines()
                        .into_iter()
                        .map(|line| div().child(line)),
                )
        })
    }

    pub(crate) fn image_info_lines(&self) -> Vec<String> {
        let Some(path) = self.image_path() else {
            return Vec::new();
        };

        let mut lines = Vec::new();
        lines.push(format!("Name: {}", self.image_name()));

        if let Some(metadata) = self
            .current_image
            .as_ref()
            .and_then(|image| image.metadata.as_ref())
        {
            lines.push(format!(
                "Dimensions: {} x {}",
                metadata.width, metadata.height
            ));
            if let Some(format_name) = metadata.format_name.as_deref() {
                lines.push(format!("Format: {format_name}"));
            }
        } else {
            lines.push("Dimensions: unknown".to_string());
        }

        if let Ok(file_metadata) = fs::metadata(path) {
            lines.push(format!(
                "File size: {}",
                format_file_size(file_metadata.len())
            ));
            if let Ok(modified) = file_metadata.modified() {
                lines.push(format!("Modified: {}", format_modified_time(modified)));
            }
        }

        lines.push(format!("Zoom: {:.0}%", self.viewport.zoom * 100.0));
        lines.push(format!("Path: {}", path.display()));
        lines
    }
}
