use gpui::{
    div, point, px, rgb, AnyElement, Context, InteractiveElement, IntoElement, Keystroke,
    MouseButton, ParentElement, Pixels, Point, Styled, Window,
};
use gpui_component::{Icon, IconName};
use lumia_core::{SettingsGroup, ShortcutId};

use crate::app::LumiaApp;
use crate::i18n::{tr, TextKey};
use crate::palette::Palette;
use crate::plugin_panel::language_code;
use crate::widgets::{context_menu_item, context_menu_item_enabled, CONTEXT_MENU_ITEM_HEIGHT};
use crate::{
    EDIT_PANEL_WIDTH, STATUS_BAR_HEIGHT, STATUS_MENU_BOTTOM, ZOOM_MENU_ITEM_HEIGHT,
    ZOOM_MENU_RIGHT, ZOOM_MENU_WIDTH,
};

const CONTEXT_MENU_WIDTH: f32 = 192.0;
const CONTEXT_MENU_MARGIN: f32 = 8.0;

impl LumiaApp {
    pub(crate) fn clamped_context_menu_position(
        &self,
        pointer: Point<Pixels>,
        window: &Window,
    ) -> Point<Pixels> {
        let viewport = window.viewport_size();
        let plugin_item_count = self
            .plugins
            .context_menu_items(
                language_code(self.settings.language),
                self.viewer.has_document(),
                self.plugin_canvas_available(),
            )
            .len();
        let right_inset = if self.editing.mode.is_some() {
            EDIT_PANEL_WIDTH
        } else if self.plugins.active.is_some() {
            crate::PLUGIN_PANEL_WIDTH
        } else {
            0.0
        };
        let bottom_inset = if self.status_bar_visible() {
            STATUS_BAR_HEIGHT
        } else {
            0.0
        };
        point(
            px(clamp_menu_coordinate(
                f32::from(pointer.x),
                f32::from(viewport.width),
                CONTEXT_MENU_WIDTH,
                right_inset,
            )),
            px(clamp_menu_coordinate(
                f32::from(pointer.y),
                f32::from(viewport.height),
                context_menu_height(plugin_item_count),
                bottom_inset,
            )),
        )
    }

    pub(crate) fn render_zoom_menu(
        &self,
        palette: Palette,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !self.ui.show_zoom_menu {
            return None;
        }

        // Center the menu under the zoom button, like the edit menu. Fall back
        // to the old right-edge position before the button has been measured.
        let menu_left = self.ui.zoom_menu_anchor.map_or_else(
            || f32::from(window.viewport_size().width) - ZOOM_MENU_RIGHT - ZOOM_MENU_WIDTH,
            |anchor| {
                f32::from(anchor.left()) + (f32::from(anchor.size.width) - ZOOM_MENU_WIDTH) / 2.0
            },
        );

        let presets = [32.0, 16.0, 8.0, 4.0, 2.0, 1.5, 1.0, 0.5, 0.1];
        Some(
            div()
                .id("status-zoom-menu")
                .absolute()
                .left(px(menu_left))
                .bottom(px(STATUS_MENU_BOTTOM))
                .w(px(ZOOM_MENU_WIDTH))
                .py_2()
                .rounded_md()
                .border_1()
                .border_color(rgb(palette.border))
                .bg(rgb(palette.panel_bg))
                .shadow_lg()
                .children(presets.into_iter().map(|zoom| {
                    let active = (self.viewer.viewport().zoom - zoom).abs() < 0.01;
                    self.render_zoom_menu_item(zoom, active, palette, cx)
                }))
                .into_any_element(),
        )
    }

    fn render_zoom_menu_item(
        &self,
        zoom: f32,
        active: bool,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .id(format!("zoom-preset-{:.0}", zoom * 100.0))
            .w_full()
            .h(px(ZOOM_MENU_ITEM_HEIGHT))
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .text_sm()
            .text_color(rgb(if active {
                palette.accent
            } else {
                palette.muted_text
            }))
            .hover(move |style| style.bg(rgb(palette.button_hover)))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, window, cx| {
                    this.set_zoom(zoom, window, cx);
                }),
            )
            .child(
                div()
                    .w(px(16.0))
                    .h(px(16.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .children(active.then(|| {
                        Icon::new(IconName::Check)
                            .size(px(14.0))
                            .text_color(rgb(palette.accent))
                    })),
            )
            .child(format!("{:.0}%", zoom * 100.0))
            .into_any_element()
    }
    pub(crate) fn render_context_menu(
        &self,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let language = self.settings.language;
        let has_image = self.viewer.has_document();
        // `metadata` is populated for every supported format, so check for
        // actual EXIF fields rather than mere metadata presence — otherwise
        // the copy action would silently produce empty text for images
        // without camera EXIF data.
        let has_exif = !self.exif_only_lines(language).is_empty();
        let slideshow_active = self.slideshow.is_active();
        let slideshow_enabled = slideshow_active || self.can_start_slideshow();
        let slideshow_label = if slideshow_active {
            TextKey::StopSlideshow
        } else {
            TextKey::Slideshow
        };
        let fullscreen_label = if self.ui.is_fullscreen {
            TextKey::ExitFullscreen
        } else {
            TextKey::Fullscreen
        };
        let plugin_items = self.plugins.context_menu_items(
            language_code(self.settings.language),
            self.viewer.has_document(),
            self.plugin_canvas_available(),
        );
        let plugin_elements = self.render_plugin_context_menu_items(plugin_items, palette, cx);

        self.ui.context_menu_position.map(|position| {
            div()
                .id("viewer-context-menu")
                .absolute()
                .left(position.x)
                .top(position.y)
                .w(px(CONTEXT_MENU_WIDTH))
                .py_1()
                .rounded_md()
                .bg(rgb(palette.panel_bg))
                .border_1()
                .border_color(rgb(palette.border))
                .shadow_lg()
                .text_color(rgb(palette.text))
                .text_sm()
                .child(context_menu_item(
                    "open-menu-item",
                    tr(language, TextKey::Open),
                    Keystroke::parse(&self.get_shortcut_binding(ShortcutId::OpenFile)).ok(),
                    palette,
                    cx,
                    |this, _, _, cx| {
                        this.open_file_dialog(cx, None);
                    },
                ))
                .child(context_menu_item(
                    "open-url-menu-item",
                    tr(language, TextKey::OpenUrl),
                    None,
                    palette,
                    cx,
                    |this, _, window, cx| {
                        this.open_url_dialog(window, cx);
                    },
                ))
                .child(context_menu_item_enabled(
                    "slideshow-menu-item",
                    tr(language, slideshow_label),
                    slideshow_enabled,
                    None,
                    palette,
                    cx,
                    |this, _, window, cx| {
                        this.toggle_slideshow(window, cx);
                    },
                ))
                .child(context_menu_item(
                    "fullscreen-menu-item",
                    tr(language, fullscreen_label),
                    Keystroke::parse(&self.get_shortcut_binding(ShortcutId::ToggleFullscreen)).ok(),
                    palette,
                    cx,
                    |this, _, window, cx| {
                        this.toggle_window_fullscreen(window, cx);
                    },
                ))
                .children(plugin_elements)
                .child(div().h(px(1.0)).my_1().bg(rgb(palette.border)))
                .child(context_menu_item_enabled(
                    "copy-exif-menu-item",
                    tr(language, TextKey::CopyExifInfo),
                    has_exif,
                    None,
                    palette,
                    cx,
                    |this, _, _, cx| {
                        this.copy_exif_info(cx);
                    },
                ))
                .child(context_menu_item_enabled(
                    "show-exif-menu-item",
                    tr(language, TextKey::ShowExifInfo),
                    has_image,
                    None,
                    palette,
                    cx,
                    |this, _, _, cx| {
                        this.ui.show_image_info = true;
                        this.ui.context_menu_position = None;
                        cx.notify();
                    },
                ))
                .child(context_menu_item_enabled(
                    "copy-file-path-menu-item",
                    tr(language, TextKey::CopyFilePath),
                    has_image,
                    None,
                    palette,
                    cx,
                    |this, _, _, cx| {
                        this.copy_file_path(cx);
                    },
                ))
                .child(context_menu_item_enabled(
                    "open-file-location-menu-item",
                    tr(language, TextKey::OpenFileLocation),
                    has_image,
                    None,
                    palette,
                    cx,
                    |this, _, _, cx| {
                        this.open_file_location(cx);
                    },
                ))
                .child(context_menu_item_enabled(
                    "delete-menu-item",
                    tr(language, TextKey::Delete),
                    has_image,
                    None,
                    palette,
                    cx,
                    |this, _, window, cx| {
                        this.delete_current_image(window, cx);
                    },
                ))
                .child(div().h(px(1.0)).my_1().bg(rgb(palette.border)))
                .child(context_menu_item(
                    "settings-menu-item",
                    tr(language, TextKey::Settings),
                    Keystroke::parse(&self.get_shortcut_binding(ShortcutId::OpenSettings)).ok(),
                    palette,
                    cx,
                    |this, _, _, cx| {
                        this.open_settings_panel(cx);
                    },
                ))
                .child(context_menu_item(
                    "about-menu-item",
                    tr(language, TextKey::About),
                    Keystroke::parse(&self.get_shortcut_binding(ShortcutId::About)).ok(),
                    palette,
                    cx,
                    |this, _, _, cx| {
                        this.open_settings_panel_to(SettingsGroup::About, cx);
                    },
                ))
                .child(div().h(px(1.0)).my_1().bg(rgb(palette.border)))
                .child(context_menu_item(
                    "quit-menu-item",
                    tr(language, TextKey::Quit),
                    Keystroke::parse(&self.get_shortcut_binding(ShortcutId::Quit)).ok(),
                    palette,
                    cx,
                    |this, _, _, cx| {
                        this.ui.context_menu_position = None;
                        cx.quit();
                    },
                ))
        })
    }
}

fn context_menu_height(plugin_item_count: usize) -> f32 {
    2.0 + 8.0 + (12 + plugin_item_count) as f32 * CONTEXT_MENU_ITEM_HEIGHT + 3.0 * 9.0
}

fn clamp_menu_coordinate(
    pointer: f32,
    viewport_extent: f32,
    menu_extent: f32,
    trailing_inset: f32,
) -> f32 {
    let maximum = (viewport_extent - trailing_inset - menu_extent - CONTEXT_MENU_MARGIN).max(0.0);
    pointer.clamp(CONTEXT_MENU_MARGIN.min(maximum), maximum)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_menu_coordinate_stays_inside_trailing_edge() {
        assert_eq!(
            clamp_menu_coordinate(790.0, 800.0, CONTEXT_MENU_WIDTH, 0.0),
            600.0
        );
        assert_eq!(
            clamp_menu_coordinate(590.0, 600.0, context_menu_height(0), STATUS_BAR_HEIGHT),
            183.0
        );
    }

    #[test]
    fn context_menu_coordinate_preserves_safe_positions_and_small_viewports() {
        assert_eq!(
            clamp_menu_coordinate(240.0, 800.0, CONTEXT_MENU_WIDTH, 0.0),
            240.0
        );
        assert_eq!(
            clamp_menu_coordinate(10.0, 100.0, CONTEXT_MENU_WIDTH, 0.0),
            0.0
        );
    }

    #[test]
    fn context_menu_height_grows_with_plugin_contributions() {
        assert_eq!(
            context_menu_height(2) - context_menu_height(0),
            2.0 * CONTEXT_MENU_ITEM_HEIGHT
        );
    }
}
