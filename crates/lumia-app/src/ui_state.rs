use std::path::PathBuf;

use gpui::{Bounds, Pixels, Point};
use lumia_core::{SettingsGroup, ShortcutId};

use crate::file_association_state::FileAssociationUiState;
use crate::update_check::UpdateCheckUiState;

pub(crate) struct UiState {
    pub(crate) error_message: Option<String>,
    pub(crate) pending_drop_paths: Vec<PathBuf>,
    pub(crate) is_panning: bool,
    pub(crate) is_overview_panning: bool,
    pub(crate) is_fullscreen: bool,
    pub(crate) show_image_info: bool,
    pub(crate) context_menu_position: Option<Point<Pixels>>,
    pub(crate) last_mouse_position: Option<Point<Pixels>>,
    pub(crate) show_settings_panel: bool,
    pub(crate) active_settings_group: SettingsGroup,
    pub(crate) file_associations: FileAssociationUiState,
    pub(crate) update_check: UpdateCheckUiState,
    pub(crate) recording_shortcut: Option<ShortcutId>,
    pub(crate) show_zoom_menu: bool,
    /// Screen-space bounds of the zoom button, used to center the zoom menu
    /// under it and to keep the menu open while the pointer hovers it.
    pub(crate) zoom_menu_anchor: Option<Bounds<Pixels>>,
    pub(crate) show_status_bar: bool,
    pub(crate) status_bar_locked: bool,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            error_message: None,
            pending_drop_paths: Vec::new(),
            is_panning: false,
            is_overview_panning: false,
            is_fullscreen: false,
            show_image_info: false,
            context_menu_position: None,
            last_mouse_position: None,
            show_settings_panel: false,
            active_settings_group: SettingsGroup::General,
            file_associations: FileAssociationUiState::default(),
            update_check: UpdateCheckUiState::default(),
            recording_shortcut: None,
            show_zoom_menu: false,
            zoom_menu_anchor: None,
            show_status_bar: false,
            status_bar_locked: false,
        }
    }
}
