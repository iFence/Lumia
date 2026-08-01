use gpui::{Bounds, Context, MouseMoveEvent, Pixels, Window};
use lumia_core::SettingsGroup;

use crate::app::LumiaApp;
use crate::{
    About, ExitFullscreen, OpenSettings, ToggleFullscreen, ToggleImageInfo, EDIT_MENU_HEIGHT,
    EDIT_MENU_WIDTH,
    STATUS_BAR_HEIGHT, STATUS_MENU_BOTTOM, STATUS_MENU_CONTROL_OFFSET, ZOOM_MENU_HEIGHT,
    ZOOM_MENU_HOVER_MARGIN, ZOOM_MENU_RIGHT, ZOOM_MENU_WIDTH,
};

impl LumiaApp {
    pub(crate) fn toggle_fullscreen(
        &mut self,
        _: &ToggleFullscreen,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_viewer_blocked() {
            self.toggle_window_fullscreen(window, cx);
        }
    }

    pub(crate) fn toggle_window_fullscreen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.toggle_fullscreen();
        self.ui.is_fullscreen = !self.ui.is_fullscreen;
        cx.notify();
    }

    pub(crate) fn exit_fullscreen(
        &mut self,
        _: &ExitFullscreen,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.stop_slideshow(cx) {
            return;
        }
        if self.is_viewer_blocked() {
            return;
        }
        if self.ui.is_fullscreen || window.is_fullscreen() {
            window.toggle_fullscreen();
            self.ui.is_fullscreen = false;
            cx.notify();
        }
    }

    pub(crate) fn toggle_image_info(
        &mut self,
        _: &ToggleImageInfo,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_viewer_blocked() {
            return;
        }
        self.ui.show_image_info = !self.ui.show_image_info;
        cx.notify();
    }

    pub(crate) fn open_settings(
        &mut self,
        _: &OpenSettings,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_viewer_blocked() {
            self.open_settings_panel(cx);
        }
    }

    pub(crate) fn open_about(
        &mut self,
        _: &About,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_viewer_blocked() {
            self.open_settings_panel_to(SettingsGroup::About, cx);
        }
    }

    pub(crate) fn open_settings_panel_to(
        &mut self,
        group: SettingsGroup,
        cx: &mut Context<Self>,
    ) {
        self.stop_slideshow(cx);
        self.close_plugin_session(cx);
        self.ui.show_settings_panel = true;
        self.ui.active_settings_group = group;
        self.ui.file_associations.initialized = false;
        self.ui.file_associations.feedback = None;
        self.ui.context_menu_position = None;
        cx.notify();
    }

    pub(crate) fn open_settings_panel(&mut self, cx: &mut Context<Self>) {
        self.open_settings_panel_to(SettingsGroup::General, cx);
    }

    pub(crate) fn close_settings_panel(&mut self, cx: &mut Context<Self>) {
        self.ui.show_settings_panel = false;
        self.ui.recording_shortcut = None;
        cx.notify();
    }

    pub(crate) fn select_settings_group(&mut self, group: SettingsGroup, cx: &mut Context<Self>) {
        self.ui.active_settings_group = group;
        if group == SettingsGroup::FileAssociations && !self.ui.file_associations.initialized {
            self.initialize_file_associations(cx);
        }
        cx.notify();
    }

    pub(crate) fn handle_root_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let viewport_size = window.viewport_size();
        let viewport_width = f32::from(viewport_size.width);
        let viewport_height = f32::from(viewport_size.height);
        let x = f32::from(event.position.x);
        let y = f32::from(event.position.y);
        let in_zoom_menu_zone = self.ui.show_zoom_menu && {
            match self.ui.zoom_menu_anchor {
                // Not measured yet, right after opening — use the old position.
                None => {
                    let menu_left = viewport_width - ZOOM_MENU_RIGHT - ZOOM_MENU_WIDTH;
                    let menu_right = viewport_width - ZOOM_MENU_RIGHT;
                    let menu_top = viewport_height - STATUS_MENU_BOTTOM - ZOOM_MENU_HEIGHT;
                    x >= menu_left - ZOOM_MENU_HOVER_MARGIN
                        && x <= menu_right + ZOOM_MENU_HOVER_MARGIN
                        && y >= menu_top - ZOOM_MENU_HOVER_MARGIN
                        && y <= viewport_height
                }
                Some(anchor) => zoom_menu_zone_contains(anchor, x, y, viewport_height),
            }
        };
        // The edit menu behaves like the zoom menu: it stays open while the
        // pointer hovers it (and the status bar), and closes once it leaves.
        let in_edit_menu_zone = self.editing.show_menu && {
            match self.editing.menu_anchor {
                // Not measured yet, right after opening — keep it open.
                None => true,
                Some(anchor) => {
                    edit_menu_zone_contains(anchor, x, y, viewport_height)
                }
            }
        };
        let show_status_bar = should_show_status_bar(
            self.ui.status_bar_locked,
            y,
            viewport_height,
            in_zoom_menu_zone || in_edit_menu_zone,
        );
        let mut changed = false;
        if self.ui.show_status_bar != show_status_bar {
            self.ui.show_status_bar = show_status_bar;
            changed = true;
        }
        if self.ui.show_zoom_menu && !in_zoom_menu_zone {
            self.ui.show_zoom_menu = false;
            changed = true;
        }
        if self.editing.show_menu && !in_edit_menu_zone {
            self.editing.show_menu = false;
            changed = true;
        }
        if changed {
            cx.notify();
        }
    }
}

fn zoom_menu_zone_contains(
    anchor: Bounds<Pixels>,
    x: f32,
    y: f32,
    viewport_height: f32,
) -> bool {
    // Mirror the menu's centering under the zoom button in status_bar.rs.
    let menu_left = f32::from(anchor.left()) + (f32::from(anchor.size.width) - ZOOM_MENU_WIDTH) / 2.0;
    let menu_right = menu_left + ZOOM_MENU_WIDTH;
    let menu_top = viewport_height - STATUS_MENU_BOTTOM - ZOOM_MENU_HEIGHT;
    x >= menu_left - ZOOM_MENU_HOVER_MARGIN
        && x <= menu_right + ZOOM_MENU_HOVER_MARGIN
        && y >= menu_top - ZOOM_MENU_HOVER_MARGIN
        && y <= viewport_height
}

fn edit_menu_zone_contains(
    anchor: Bounds<Pixels>,
    x: f32,
    y: f32,
    viewport_height: f32,
) -> bool {
    // Mirror the popup's centering under the dimensions button in status_bar.rs.
    let menu_left = f32::from(anchor.left())
        + (f32::from(anchor.size.width) - EDIT_MENU_WIDTH) / 2.0;
    let menu_right = menu_left + EDIT_MENU_WIDTH;
    let menu_top = f32::from(anchor.bottom()) - STATUS_MENU_CONTROL_OFFSET - EDIT_MENU_HEIGHT
        - ZOOM_MENU_HOVER_MARGIN;
    x >= menu_left - ZOOM_MENU_HOVER_MARGIN
        && x <= menu_right + ZOOM_MENU_HOVER_MARGIN
        && y >= menu_top - ZOOM_MENU_HOVER_MARGIN
        && y <= viewport_height
}

fn should_show_status_bar(
    locked: bool,
    pointer_y: f32,
    viewport_height: f32,
    in_zoom_menu_zone: bool,
) -> bool {
    locked || in_zoom_menu_zone || pointer_y >= (viewport_height - STATUS_BAR_HEIGHT).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlocked_status_bar_only_appears_in_bottom_hover_zone() {
        assert!(!should_show_status_bar(false, 700.0, 800.0, false));
        assert!(should_show_status_bar(false, 764.0, 800.0, false));
        assert!(should_show_status_bar(false, 700.0, 800.0, true));
    }

    #[test]
    fn locked_status_bar_remains_visible() {
        assert!(should_show_status_bar(true, 0.0, 800.0, false));
    }
}
