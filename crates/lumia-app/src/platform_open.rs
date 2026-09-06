use std::path::PathBuf;

use async_channel::Receiver;
use gpui::App;

pub(crate) fn file_path(url: &str) -> Option<PathBuf> {
    url::Url::parse(url).ok()?.to_file_path().ok()
}

pub(crate) fn listen(receiver: Receiver<PathBuf>, cx: &mut App) {
    cx.spawn(async move |cx| {
        while let Ok(path) = receiver.recv().await {
            cx.update(|cx| crate::bootstrap::open_viewer_window(Some(path), cx));
        }
    })
    .detach();
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
