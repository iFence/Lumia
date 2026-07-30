#![windows_subsystem = "windows"]

mod annotation_export;
mod annotation_overlay;
mod app;
mod bootstrap;
mod cli;
mod common_decode;
mod crop_overlay;
mod custom_icons;
mod editing;
mod editing_export;
mod editing_panel;
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
mod platform_open;
mod plugin_catalog;
mod plugin_controls;
mod plugin_installation;
mod plugin_management;
mod plugin_package;
mod plugin_panel;
mod plugin_state;
mod preferences;
mod preview_cache;
mod professional_decode;
mod professional_preview;
mod render;
mod settings_about;
mod settings_association_formats;
mod settings_associations;
mod settings_general;
mod settings_plugins;
mod settings_shortcuts;
mod settings_ui;
mod shell;
mod single_instance;
mod slideshow;
mod status_bar;
mod tile_cache;
mod ui_state;
mod update_check;
mod util;
mod viewer_actions;
mod viewer_files;
mod viewer_overlays;
mod widgets;
mod window_actions;

use gpui::{actions, Action};
use lumia_core::{Language, ThemeAccent, ThemeMode};
use serde::Deserialize;

pub(crate) const STATUS_BAR_HEIGHT: f32 = 36.0;
pub(crate) const STATUS_CONTROL_HEIGHT: f32 = 24.0;
pub(crate) const STATUS_MENU_BOTTOM: f32 = (STATUS_BAR_HEIGHT + STATUS_CONTROL_HEIGHT) / 2.0;
pub(crate) const EDIT_PANEL_WIDTH: f32 = 320.0;
pub(crate) const PLUGIN_PANEL_WIDTH: f32 = 288.0;
pub(crate) const ZOOM_BUTTON_WIDTH: f32 = 80.0;
pub(crate) const ZOOM_MENU_WIDTH: f32 = 148.0;
pub(crate) const ZOOM_MENU_RIGHT: f32 = 48.0;
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
        CheckForUpdates,
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
        cli::CliCommand::VerifyPluginPackage(path) => {
            let package = plugin_package::verify_official_package_file(&path)?;
            println!(
                "Verified official plugin package {} {} for {}/{}.",
                package.manifest.plugin_id,
                package.manifest.version,
                package.manifest.target_os,
                package.manifest.target_arch
            );
            Ok(())
        }
        #[cfg(target_os = "windows")]
        cli::CliCommand::RepairFileAssociations => shell::repair_legacy_file_associations(),
        cli::CliCommand::OpenFile(path) => bootstrap::run_gui(Some(path)),
        cli::CliCommand::Normal => {
            #[cfg(target_os = "windows")]
            ensure_file_associations_on_first_launch();
            bootstrap::run_gui(None)
        }
    }
}

#[cfg(target_os = "windows")]
fn ensure_file_associations_on_first_launch() {
    match shell::query_file_associations() {
        Ok(snapshot) if snapshot.configured => {}
        _ => {
            let extensions = lumia_core::supported_image_extensions()
                .iter()
                .map(|ext| (*ext).to_string())
                .collect::<std::collections::BTreeSet<_>>();
            if let Err(err) = shell::apply_file_associations(&extensions) {
                log_windows_error("first_launch_file_associations", &err);
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn log_windows_error(context: &str, error: &anyhow::Error) {
    let message = format!("[{context}] {error}\n");
    if let Ok(temp) = std::env::var("TEMP") {
        let dir = std::path::PathBuf::from(temp).join("Lumia");
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("debug.log"), message);
    }
}
