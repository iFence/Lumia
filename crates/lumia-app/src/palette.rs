use gpui::{Window, WindowAppearance};
use lumia_core::ThemeMode;

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
    pub(crate) selection_bg: u32,
    pub(crate) border: u32,
    pub(crate) text: u32,
    pub(crate) muted_text: u32,
    pub(crate) error_text: u32,
}

impl Palette {
    fn for_theme(theme: ThemeMode, appearance: WindowAppearance) -> Self {
        if theme_resolves_to_dark(theme, appearance) {
            Self::dark()
        } else {
            Self::light()
        }
    }

    fn dark() -> Self {
        Self {
            viewer_bg: 0x202020,
            toolbar_bg: 0x181818,
            panel_bg: 0x252525,
            sidebar_bg: 0x202020,
            subtle_bg: 0x2c2c2c,
            button_bg: 0x2f2f2f,
            button_hover: 0x3a3a3a,
            selection_bg: 0x3d4a5c,
            border: 0x3c3c3c,
            text: 0xf2f2f2,
            muted_text: 0xbdbdbd,
            error_text: 0xffb3b3,
        }
    }

    fn light() -> Self {
        Self {
            viewer_bg: 0xf4f4f4,
            toolbar_bg: 0xffffff,
            panel_bg: 0xffffff,
            sidebar_bg: 0xf2f2f2,
            subtle_bg: 0xf5f5f5,
            button_bg: 0xeeeeee,
            button_hover: 0xe2e2e2,
            selection_bg: 0xdce8f8,
            border: 0xd0d0d0,
            text: 0x202020,
            muted_text: 0x5f6368,
            error_text: 0x9b1c1c,
        }
    }
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
        Palette::for_theme(self.settings.theme, window.appearance())
    }
}
