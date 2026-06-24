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

use gpui::{
    actions, px, size, App, AppContext, Application, Bounds, WindowBounds, WindowOptions,
};

pub(crate) const TOOLBAR_HEIGHT: f32 = 36.0;
pub(crate) const TITLE_BAR_HEIGHT: f32 = 24.0;
pub(crate) const APP_TITLE: &str = "Lumia";

actions!(
    lumia,
    [
        OpenFile,
        NextImage,
        PreviousImage,
        ZoomIn,
        ZoomOut,
        ZoomFit,
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
    Application::new().run(move |cx: &mut App| {
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
                cx.new(|cx| app::LumiaApp::new(window, cx, initial_path.clone()))
            },
        )
        .expect("failed to open Lumia window");
        cx.activate(true);
    });

    Ok(())
}
