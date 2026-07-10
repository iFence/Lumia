use std::path::PathBuf;

use gpui::{px, size, App, AppContext, Bounds, WindowBounds, WindowOptions};

use crate::{app::LumiaApp, Quit, APP_TITLE};

pub(crate) fn run_gui(initial_path: Option<PathBuf>) -> anyhow::Result<()> {
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
                    let view = cx.new(|cx| LumiaApp::new(window, cx, initial_path.clone()));
                    view.update(cx, |app, cx| app.set_self_handle(view.downgrade(), cx));
                    cx.new(|cx| gpui_component::Root::new(view, window, cx).bordered(false))
                },
            )
            .expect("failed to open Lumia window");
            cx.activate(true);
        });

    Ok(())
}
