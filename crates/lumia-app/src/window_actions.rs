use gpui::{Context, MouseMoveEvent, Window};
use lumia_core::SettingsGroup;

use crate::app::LumiaApp;
use crate::{
    ExitFullscreen, ToggleFullscreen, ToggleImageInfo, STATUS_BAR_HEIGHT, ZOOM_MENU_BOTTOM_GAP,
    ZOOM_MENU_HEIGHT, ZOOM_MENU_HOVER_MARGIN, ZOOM_MENU_RIGHT, ZOOM_MENU_WIDTH,
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

    pub(crate) fn open_settings_panel(&mut self, cx: &mut Context<Self>) {
        self.ui.show_settings_panel = true;
        self.ui.active_settings_group = SettingsGroup::General;
        #[cfg(target_os = "windows")]
        {
            self.ui.file_associations.initialized = false;
            self.ui.file_associations.feedback = None;
        }
        self.ui.context_menu_position = None;
        cx.notify();
    }

    pub(crate) fn close_settings_panel(&mut self, cx: &mut Context<Self>) {
        self.ui.show_settings_panel = false;
        self.ui.recording_shortcut = None;
        cx.notify();
    }

    pub(crate) fn select_settings_group(&mut self, group: SettingsGroup, cx: &mut Context<Self>) {
        self.ui.active_settings_group = group;
        #[cfg(target_os = "windows")]
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
        // The status bar is pinned visible by default, so it is never
        // auto-hidden. Only the zoom menu is dismissed when the pointer
        // leaves its hover zone.
        let in_zoom_menu_zone = self.ui.show_zoom_menu && {
            let menu_left = viewport_width - ZOOM_MENU_RIGHT - ZOOM_MENU_WIDTH;
            let menu_right = viewport_width - ZOOM_MENU_RIGHT;
            let menu_top =
                viewport_height - STATUS_BAR_HEIGHT - ZOOM_MENU_BOTTOM_GAP - ZOOM_MENU_HEIGHT;
            let menu_bottom = viewport_height - STATUS_BAR_HEIGHT;
            x >= menu_left - ZOOM_MENU_HOVER_MARGIN
                && x <= menu_right + ZOOM_MENU_HOVER_MARGIN
                && y >= menu_top - ZOOM_MENU_HOVER_MARGIN
                && y <= menu_bottom + ZOOM_MENU_HOVER_MARGIN
        };
        if self.ui.show_zoom_menu && !in_zoom_menu_zone {
            self.ui.show_zoom_menu = false;
            cx.notify();
        }
    }
}
