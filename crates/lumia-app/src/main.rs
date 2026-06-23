#![windows_subsystem = "windows"]

mod app;
mod i18n;
mod image_info;
mod palette;
mod persistence;
mod render;
mod settings_ui;
mod util;
mod widgets;

use gpui::{
    actions, px, size, App, AppContext, Application, Bounds, WindowBounds, WindowOptions,
};

pub(crate) const TOOLBAR_HEIGHT: f32 = 48.0;
const APP_TITLE: &str = "Lumia";

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
    Application::new().run(|cx: &mut App| {
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
            |window, cx| cx.new(|cx| app::LumiaApp::new(window, cx)),
        )
        .expect("failed to open Lumia window");
        cx.activate(true);
    });

    Ok(())
}
