use gpui::Window;
use lumia_core::ThemeAccent;

use crate::app::LumiaApp;

#[derive(Clone, Copy)]
pub(crate) struct Palette {
    pub(crate) viewer_bg: u32,
    pub(crate) toolbar_bg: u32,
    pub(crate) toolbar_bg_alpha: f32,
    pub(crate) panel_bg: u32,
    pub(crate) sidebar_bg: u32,
    pub(crate) subtle_bg: u32,
    pub(crate) button_hover: u32,
    pub(crate) status_hover: u32,
    pub(crate) border: u32,
    pub(crate) text: u32,
    pub(crate) muted_text: u32,
    pub(crate) error_text: u32,
    pub(crate) accent: u32,
    pub(crate) accent_hover: u32,
    pub(crate) accent_active: u32,
    pub(crate) accent_text: u32,
    pub(crate) accent_soft: u32,
}

impl Palette {
    fn dark(accent: ThemeAccent) -> Self {
        let accent = accent_color(accent);

        Self {
            viewer_bg: 0x202020,
            toolbar_bg: 0x181818,
            toolbar_bg_alpha: 0.72,
            panel_bg: 0x252525,
            sidebar_bg: 0x202020,
            subtle_bg: 0x2c2c2c,
            button_hover: mix(0x3a3a3a, accent, 0.28),
            status_hover: 0x2a2a2a,
            border: 0x3c3c3c,
            text: 0xf2f2f2,
            muted_text: 0xbdbdbd,
            error_text: 0xffb3b3,
            accent,
            accent_hover: mix(accent, 0xffffff, 0.14),
            accent_active: mix(accent, 0x000000, 0.16),
            accent_text: 0x101010,
            accent_soft: mix(0x252525, accent, 0.30),
        }
    }
}

fn accent_color(accent: ThemeAccent) -> u32 {
    match accent {
        ThemeAccent::Blue => 0x2389da,
        ThemeAccent::Green => 0x2a9d8f,
        ThemeAccent::Orange => 0xf2a900,
        ThemeAccent::Rose => 0xe56b8c,
    }
}

fn mix(base: u32, overlay: u32, amount: f32) -> u32 {
    let amount = amount.clamp(0.0, 1.0);
    let base_r = ((base >> 16) & 0xff) as f32;
    let base_g = ((base >> 8) & 0xff) as f32;
    let base_b = (base & 0xff) as f32;
    let overlay_r = ((overlay >> 16) & 0xff) as f32;
    let overlay_g = ((overlay >> 8) & 0xff) as f32;
    let overlay_b = (overlay & 0xff) as f32;

    let r = (base_r + (overlay_r - base_r) * amount).round() as u32;
    let g = (base_g + (overlay_g - base_g) * amount).round() as u32;
    let b = (base_b + (overlay_b - base_b) * amount).round() as u32;

    (r << 16) | (g << 8) | b
}

impl LumiaApp {
    pub(crate) fn palette(&self, _window: &Window) -> Palette {
        Palette::dark(self.settings.theme_accent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accent_colors_use_named_palette_values() {
        assert_eq!(accent_color(ThemeAccent::Blue), 0x2389da);
        assert_eq!(accent_color(ThemeAccent::Green), 0x2a9d8f);
        assert_eq!(accent_color(ThemeAccent::Orange), 0xf2a900);
        assert_eq!(accent_color(ThemeAccent::Rose), 0xe56b8c);
    }
}
