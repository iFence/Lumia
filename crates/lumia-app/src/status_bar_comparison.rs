//! Status-bar readout for comparison mode.
//!
//! Each pane gets a chip carrying its folder position, pixel dimensions and
//! file size, so both sides of the comparison are legible at a glance instead
//! of the bar reporting the left image's figures with the right image's name
//! beside them. Clicking a chip targets that pane for transform commands.

use gpui_kit::{
    div, px, rgb, Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Styled,
};

use crate::app::LumiaApp;
use crate::comparison::PaneSummary;
use crate::palette::Palette;
use crate::util::format_file_size;

impl LumiaApp {
    /// The two pane chips, rendered in place of the single folder position
    /// while comparing.
    pub(crate) fn render_comparison_summaries(
        &self,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> Option<gpui_kit::AnyElement> {
        let summaries = self.comparison_pane_summaries()?;
        let mut row = div().flex().items_center().gap_1();
        for pane in summaries {
            row = row.child(render_pane_chip(pane, palette, cx));
        }
        Some(row.into_any_element())
    }
}

fn render_pane_chip(
    pane: PaneSummary,
    palette: Palette,
    cx: &mut Context<LumiaApp>,
) -> impl IntoElement {
    let id: &'static str = if pane.label == "B" {
        "status-pane-chip-b"
    } else {
        "status-pane-chip-a"
    };
    let mut chip = div()
        .id(id)
        .flex()
        .items_center()
        .gap_1()
        .px_2()
        .h(px(24.0))
        .rounded_sm()
        .cursor_pointer()
        .text_sm()
        .hover(move |style| style.bg(rgb(palette.status_hover)));
    if pane.selected {
        chip = chip.border_1().border_color(rgb(palette.accent));
    }
    let right = pane.label == "B";
    chip.child(
        div()
            .text_color(rgb(palette.accent))
            .child(pane.label.to_string()),
    )
    .child(chip_text(chip_position(pane.position), palette))
    .child(chip_text(chip_dimensions(pane.dimensions), palette))
    .child(chip_text(chip_file_size(pane.file_size), palette))
    .on_mouse_down(
        MouseButton::Left,
        cx.listener(move |this, _, _, cx| {
            this.select_comparison_pane(right, cx);
        }),
    )
}

fn chip_text(text: String, palette: Palette) -> impl IntoElement {
    div()
        .text_sm()
        .text_color(rgb(palette.muted_text))
        .child(text)
}

fn chip_position(position: Option<(usize, usize)>) -> String {
    match position {
        Some((current, total)) => format!("{current}/{total}"),
        None => "--".to_string(),
    }
}

fn chip_dimensions(dimensions: Option<(u32, u32)>) -> String {
    match dimensions {
        Some((width, height)) => format!("{width}x{height}"),
        None => "--".to_string(),
    }
}

fn chip_file_size(size: Option<u64>) -> String {
    size.map_or_else(|| "--".to_string(), format_file_size)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_figures_fall_back_to_a_placeholder() {
        assert_eq!(chip_position(Some((3, 12))), "3/12");
        assert_eq!(chip_position(None), "--");
        assert_eq!(chip_dimensions(Some((3840, 2160))), "3840x2160");
        assert_eq!(chip_dimensions(None), "--");
    }

    #[test]
    fn file_size_uses_the_shared_formatter() {
        assert_eq!(chip_file_size(Some(1_048_576)), "1.0 MB");
        assert_eq!(chip_file_size(Some(420 * 1024)), "420.0 KB");
        assert_eq!(chip_file_size(None), "--");
    }
}
