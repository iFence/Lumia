use gpui::{Window, WindowAppearance};
use lumia_core::{ThemeAccent, ThemeMode};

use crate::app::LumiaApp;

#[derive(Clone, Copy)]
pub(crate) struct Palette {
    pub(crate) viewer_bg: u32,
    pub(crate) toolbar_bg: u32,
    pub(crate) panel_bg: u32,
    pub(crate) sidebar_bg: u32,
    pub(crate) subtle_bg: u32,
    pub(crate) button_bg: u32,
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
    fn for_theme(theme: ThemeMode, accent: ThemeAccent, appearance: WindowAppearance) -> Self {
        if theme_resolves_to_dark(theme, appearance) {
            Self::dark(accent)
        } else {
            Self::light(accent)
        }
    }

    fn dark(accent: ThemeAccent) -> Self {
        let accent = accent_color(accent);

        Self {
            viewer_bg: 0x202020,
            toolbar_bg: 0x181818,
            panel_bg: 0x252525,
            sidebar_bg: 0x202020,
            subtle_bg: 0x2c2c2c,
            button_bg: mix(0x2f2f2f, accent, 0.20),
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

    fn light(accent: ThemeAccent) -> Self {
        let accent = accent_color(accent);

        Self {
            viewer_bg: 0xf4f4f4,
            toolbar_bg: 0xffffff,
            panel_bg: 0xffffff,
            sidebar_bg: 0xf2f2f2,
            subtle_bg: 0xf5f5f5,
            button_bg: mix(0xeeeeee, accent, 0.10),
            button_hover: mix(0xe2e2e2, accent, 0.16),
            status_hover: 0xe8e8e8,
            border: 0xd0d0d0,
            text: 0x202020,
            muted_text: 0x5f6368,
            error_text: 0x9b1c1c,
            accent,
            accent_hover: mix(accent, 0x000000, 0.08),
            accent_active: mix(accent, 0x000000, 0.16),
            accent_text: 0x101010,
            accent_soft: mix(0xf5f5f5, accent, 0.18),
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

pub(crate) fn theme_resolves_to_dark(theme: ThemeMode, appearance: WindowAppearance) -> bool {
    match theme {
        ThemeMode::Light => false,
        ThemeMode::Dark => true,
        ThemeMode::FollowSystem => matches!(
            appearance,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ),
    }
}

impl LumiaApp {
    pub(crate) fn palette(&self, window: &Window) -> Palette {
        Palette::for_theme(
            self.settings.theme,
            self.settings.theme_accent,
            window.appearance(),
        )
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
