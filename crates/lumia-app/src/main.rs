#![windows_subsystem = "windows"]

mod app;
mod cli;
mod i18n;
mod image_info;
mod palette;
mod persistence;
mod render;
mod settings_ui;
mod shell;
mod util;
mod widgets;

use gpui::{actions, px, size, Action, App, AppContext, Bounds, WindowBounds, WindowOptions};
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
        cli::CliCommand::OpenFile(path) => run_gui(Some(path)),
        cli::CliCommand::Normal => run_gui(None),
    }
}

fn run_gui(initial_path: Option<std::path::PathBuf>) -> anyhow::Result<()> {
    gpui_platform::application()
        .with_assets(gpui_component_assets::Assets)
        .run(move |cx: &mut App| {
            gpui_component::init(cx);

            cx.on_action(|_: &Quit, cx| cx.quit());

            let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(gpui::TitlebarOptions {
                        title: Some(APP_TITLE.into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                |window, cx| {
                    let view = cx.new(|cx| app::LumiaApp::new(window, cx, initial_path.clone()));
                    view.update(cx, |app, cx| app.set_self_handle(view.downgrade(), cx));
                    cx.new(|cx| gpui_component::Root::new(view, window, cx).bordered(false))
                },
            )
            .expect("failed to open Lumia window");
            cx.activate(true);
        });

    Ok(())
}
