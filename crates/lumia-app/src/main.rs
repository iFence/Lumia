#![windows_subsystem = "windows"]

mod app;
mod bootstrap;
mod cli;
mod custom_icons;
#[cfg(target_os = "windows")]
mod file_association_actions;
mod file_association_state;
mod i18n;
mod image_info;
mod image_loading;
mod image_overview;
mod large_image;
mod large_image_render;
mod load_state;
mod palette;
mod persistence;
mod plugin_catalog;
mod preferences;
mod professional_decode;
mod render;
mod settings_about;
#[cfg(target_os = "windows")]
mod settings_associations;
mod settings_general;
mod settings_shortcuts;
mod settings_ui;
mod shell;
mod single_instance;
mod status_bar;
mod tile_cache;
mod ui_state;
mod util;
mod viewer_actions;
mod viewer_overlays;
mod widgets;
mod window_actions;

use gpui::{actions, Action};
use lumia_core::{Language, ThemeAccent, ThemeMode};
use serde::Deserialize;

pub(crate) const STATUS_BAR_HEIGHT: f32 = 36.0;
pub(crate) const ZOOM_MENU_WIDTH: f32 = 132.0;
pub(crate) const ZOOM_MENU_RIGHT: f32 = 48.0;
pub(crate) const ZOOM_MENU_BOTTOM_GAP: f32 = 8.0;
pub(crate) const ZOOM_MENU_ITEM_HEIGHT: f32 = 28.0;
pub(crate) const ZOOM_MENU_HEIGHT: f32 = 16.0 + 9.0 * ZOOM_MENU_ITEM_HEIGHT;
pub(crate) const ZOOM_MENU_HOVER_MARGIN: f32 = 12.0;
pub(crate) const APP_TITLE: &str = "Lumia";

#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = lumia, no_json)]
pub(crate) struct SelectLanguage(pub(crate) Language);

#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = lumia, no_json)]
pub(crate) struct SelectThemeMode(pub(crate) ThemeMode);

#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = lumia, no_json)]
pub(crate) struct SelectThemeAccent(pub(crate) ThemeAccent);

actions!(
    lumia,
    [
        OpenFile,
        NextImage,
        PreviousImage,
        ZoomIn,
        ZoomOut,
        ZoomFit,
        RotateClockwise,
        RotateCounterClockwise,
        ToggleFullscreen,
        ExitFullscreen,
        ToggleImageInfo,
        Quit
    ]
);

fn main() -> anyhow::Result<()> {
    let command = cli::parse();

    match command {
        cli::CliCommand::RegisterContextMenu => {
            shell::register_context_menu()?;
            println!("Lumia registered for supported image formats.");
            Ok(())
        }
        cli::CliCommand::UnregisterContextMenu => {
            shell::unregister_context_menu()?;
            println!("Lumia removed from system context menu.");
            Ok(())
        }
        #[cfg(target_os = "windows")]
        cli::CliCommand::RepairFileAssociations => shell::repair_legacy_file_associations(),
        cli::CliCommand::OpenFile(path) => bootstrap::run_gui(Some(path)),
        cli::CliCommand::Normal => bootstrap::run_gui(None),
    }
}
