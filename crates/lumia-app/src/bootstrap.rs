use std::{path::PathBuf, thread};

use async_channel;
use gpui::{px, size, App, AppContext, Bounds, WindowBounds, WindowOptions};

use crate::{
    app::LumiaApp, custom_icons::CustomAssets, large_image::large_image_cache_dir, single_instance,
    util::cleanup_large_image_cache, Quit, APP_TITLE,
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

    let (platform_open_sender, platform_open_receiver) = async_channel::unbounded();
    let application = gpui_platform::application().with_assets(CustomAssets);
    application.on_open_urls(move |urls| {
        for path in urls
            .into_iter()
            .filter_map(|url| crate::platform_open::file_path(&url))
        {
            let _ = platform_open_sender.send_blocking(path);
        }
    });
    application.run(move |cx: &mut App| {
        gpui_component::init(cx);
        #[cfg(target_os = "macos")]
        set_macos_dock_icon();
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
                    app.listen_for_platform_open_requests(platform_open_receiver, window, cx);
                });
                cx.new(|cx| gpui_component::Root::new(view, window, cx).bordered(false))
            },
        )
        .expect("failed to open Lumia window");
        cx.activate(true);
    });

    Ok(())
}

/// On macOS a bare binary launched outside a `.app` bundle shows the generic
/// "exec" Dock icon. Windows embeds its icon at build time via `winres`; the
/// equivalent for macOS is to set the application icon image at runtime through
/// AppKit. This keeps the proper Lumia icon in the Dock regardless of how the
/// binary is launched.
#[cfg(target_os = "macos")]
fn set_macos_dock_icon() {
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let bytes = include_bytes!("../resources/logo.png");
    let data = NSData::with_bytes(bytes);
    if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
        let app = NSApplication::sharedApplication(mtm);
        unsafe {
            app.setApplicationIconImage(Some(&image));
        }
    }
}
