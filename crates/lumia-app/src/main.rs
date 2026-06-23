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
    actions, px, size, App, AppContext, Application, Bounds, KeyBinding, WindowBounds,
    WindowOptions,
};

pub(crate) const TOOLBAR_HEIGHT: f32 = 48.0;
const APP_TITLE: &str = "Lumia";

actions!(
    lumia,
    [
        OpenFile,
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
        cx.bind_keys([
            KeyBinding::new("cmd-o", OpenFile, None),
            KeyBinding::new("ctrl-o", OpenFile, None),
            KeyBinding::new("cmd-plus", ZoomIn, None),
            KeyBinding::new("cmd-equals", ZoomIn, None),
            KeyBinding::new("ctrl-plus", ZoomIn, None),
            KeyBinding::new("ctrl-equals", ZoomIn, None),
            KeyBinding::new("cmd-minus", ZoomOut, None),
            KeyBinding::new("ctrl-minus", ZoomOut, None),
            KeyBinding::new("cmd-0", ZoomFit, None),
            KeyBinding::new("ctrl-0", ZoomFit, None),
            KeyBinding::new("f11", ToggleFullscreen, None),
            KeyBinding::new("cmd-enter", ToggleFullscreen, None),
            KeyBinding::new("ctrl-enter", ToggleFullscreen, None),
            KeyBinding::new("escape", ExitFullscreen, None),
            KeyBinding::new("tab", ToggleImageInfo, None),
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("ctrl-q", Quit, None),
        ]);
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
