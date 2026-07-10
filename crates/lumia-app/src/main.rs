#![windows_subsystem = "windows"]

mod app;
mod bootstrap;
mod cli;
mod i18n;
mod image_info;
mod image_loading;
mod load_state;
mod palette;
mod persistence;
mod preferences;
mod render;
mod settings_general;
mod settings_shortcuts;
mod settings_ui;
mod shell;
mod status_bar;
mod ui_state;
mod util;
mod viewer_actions;
mod viewer_overlays;
mod widgets;
mod window_actions;

use gpui::{actions, Action};
use lumia_core::{Language, ThemeAccent, ThemeMode};
use serde::Deserialize;

pub(crate) const STATUS_BAR_HEIGHT: f32 = 44.0;
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
            println!("Lumia registered in system context menu. Right-click any image to open with Lumia.");
            Ok(())
        }
        cli::CliCommand::UnregisterContextMenu => {
            shell::unregister_context_menu()?;
            println!("Lumia removed from system context menu.");
            Ok(())
        }
        cli::CliCommand::OpenFile(path) => bootstrap::run_gui(Some(path)),
        cli::CliCommand::Normal => bootstrap::run_gui(None),
    }
}
