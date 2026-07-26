use gpui::{div, rgb, FontWeight, InteractiveElement, IntoElement, ParentElement, Styled};

use crate::app::LumiaApp;
use crate::i18n::{tr, TextKey};
use crate::palette::Palette;
use crate::APP_TITLE;

impl LumiaApp {
    pub(crate) fn render_about_settings(&self, palette: Palette) -> impl IntoElement {
        let language = self.settings.language;

        div()
            .id("settings-about")
            .flex_1()
            .h_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .p_5()
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
    }
}
