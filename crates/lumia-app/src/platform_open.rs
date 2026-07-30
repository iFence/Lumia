use std::path::PathBuf;

use async_channel::Receiver;
use gpui::{Context, Window};

use crate::app::LumiaApp;

pub(crate) fn file_path(url: &str) -> Option<PathBuf> {
    url::Url::parse(url).ok()?.to_file_path().ok()
}

impl LumiaApp {
    pub(crate) fn listen_for_platform_open_requests(
        &mut self,
        receiver: Receiver<PathBuf>,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        cx.spawn_in(window, async move |this, cx| {
            while let Ok(path) = receiver.recv().await {
                let result = this.update_in(cx, |this, window, cx| {
                    window.activate_window();
                    this.load_image(path, Some(window), cx);
                    cx.notify();
                });
                if result.is_err() {
                    break;
                }
            }
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_encoded_file_urls_to_paths() {
        #[cfg(target_os = "windows")]
        let (url, expected) = (
            "file:///C:/Users/test/Pictures/sample%20image.png",
            PathBuf::from(r"C:\Users\test\Pictures\sample image.png"),
        );
        #[cfg(not(target_os = "windows"))]
        let (url, expected) = (
            "file:///Users/test/Pictures/sample%20image.png",
            PathBuf::from("/Users/test/Pictures/sample image.png"),
        );
        assert_eq!(file_path(url), Some(expected));
    }

    #[test]
    fn ignores_non_file_urls() {
        assert_eq!(file_path("https://example.com/image.png"), None);
    }
}
