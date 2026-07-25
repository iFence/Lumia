use std::{path::PathBuf, thread};

use gpui::{px, size, App, AppContext, Bounds, WindowBounds, WindowOptions};

use crate::{
    app::LumiaApp, custom_icons::CustomAssets, large_image::large_image_cache_dir,
    single_instance, util::cleanup_large_image_cache, Quit, APP_TITLE,
};

const LARGE_IMAGE_DISK_CACHE_BYTES: u64 = 8 * 1024 * 1024 * 1024;

pub(crate) fn run_gui(initial_path: Option<PathBuf>) -> anyhow::Result<()> {
    let Some(primary_instance) = single_instance::acquire(initial_path.as_deref())? else {
        return Ok(());
    };
    let cache_dir = large_image_cache_dir();
    let _ = thread::Builder::new()
        .name("lumia-large-cache-cleanup".into())
        .spawn(move || {
            let _ = cleanup_large_image_cache(&cache_dir, LARGE_IMAGE_DISK_CACHE_BYTES);
        });

    gpui_platform::application()
        .with_assets(CustomAssets)
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
                move |window, cx| {
                    let view = cx.new(|cx| LumiaApp::new(window, cx, initial_path.clone()));
                    view.update(cx, |app, cx| {
                        app.set_self_handle(view.downgrade(), cx);
                        app.listen_for_instance_requests(primary_instance, window, cx);
                    });
                    cx.new(|cx| gpui_component::Root::new(view, window, cx).bordered(false))
                },
            )
            .expect("failed to open Lumia window");
            cx.activate(true);
        });

    Ok(())
}
