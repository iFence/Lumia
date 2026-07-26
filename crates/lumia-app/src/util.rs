use chrono::{DateTime, Local};
use gpui::{div, rgb, InteractiveElement, IntoElement, ParentElement, Styled};
use lumia_core::{ImageLoadError, Language, LargeImageError};
use std::{
    fs, io,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::i18n::{tr, TextKey};

const LARGE_IMAGE_CACHE_MAX_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

pub(crate) fn status_message(
    id: &'static str,
    message: impl Into<String>,
    color: u32,
) -> impl IntoElement {
    div()
        .id(id)
        .px_4()
        .py_3()
        .text_center()
        .text_color(rgb(color))
        .child(message.into())
}

pub(crate) fn format_file_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit = 0;

    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

pub(crate) fn format_modified_time(modified: std::time::SystemTime) -> String {
    match modified.duration_since(UNIX_EPOCH) {
        Ok(duration) => {
            let secs = duration.as_secs() as i64;
            let nsecs = duration.subsec_nanos();
            match DateTime::from_timestamp(secs, nsecs) {
                Some(utc) => {
                    let local: DateTime<Local> = DateTime::from(utc);
                    local.format("%Y-%m-%d %H:%M:%S").to_string()
                }
                None => "invalid timestamp".to_string(),
            }
        }
        Err(_) => "before unix epoch".to_string(),
    }
}

pub(crate) fn format_load_error(error: &ImageLoadError) -> String {
    match error {
        ImageLoadError::UnsupportedExtension(extension) => {
            format!("Unsupported image format: .{extension}")
        }
        ImageLoadError::MissingExtension(_) => "The selected file has no extension".to_string(),
        ImageLoadError::NotFound(_) => "The selected file no longer exists".to_string(),
        ImageLoadError::NotAFile(_) => "The selected path is not a file".to_string(),
        ImageLoadError::Metadata { .. }
        | ImageLoadError::HeifMetadata { .. }
        | ImageLoadError::Io { .. } => "Could not read image metadata".to_string(),
        ImageLoadError::Cancelled => "Image loading was cancelled".to_string(),
        ImageLoadError::MemoryLimit { .. } => {
            "This image exceeds the configured memory limit".to_string()
        }
    }
}

pub(crate) fn format_large_image_error(language: Language, error: &LargeImageError) -> String {
    let key = if matches!(error, LargeImageError::InsufficientDiskSpace { .. }) {
        TextKey::LargeImageDiskSpace
    } else {
        TextKey::LargeImagePreviewFailed
    };
    tr(language, key).to_string()
}

pub(crate) fn cleanup_large_image_cache(cache_dir: &Path, max_bytes: u64) -> io::Result<()> {
    if !cache_dir.is_dir() {
        return Ok(());
    }
    let now = SystemTime::now();
    let mut cache_files = Vec::new();
    for entry in fs::read_dir(cache_dir)? {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if name.ends_with(".part") {
            let _ = fs::remove_file(path);
            continue;
        }
        if !name.ends_with(".bgra") {
            continue;
        }
        let metadata = match entry.metadata() {
            Ok(metadata) => metadata,
            Err(_) => continue,
        };
        let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
        if now
            .duration_since(modified)
            .is_ok_and(|age| age > LARGE_IMAGE_CACHE_MAX_AGE)
        {
            let _ = fs::remove_file(path);
            continue;
        }
        cache_files.push((path, metadata.len(), modified));
    }

    cache_files.sort_by_key(|(_, _, modified)| *modified);
    let mut total = cache_files.iter().map(|(_, bytes, _)| *bytes).sum::<u64>();
    for (path, bytes, _) in cache_files {
        if total <= max_bytes {
            break;
        }
        if fs::remove_file(path).is_ok() {
            total = total.saturating_sub(bytes);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, time::SystemTime};

    use lumia_core::{Language, LargeImageError};

    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("lumia-cache-cleanup-{nonce}-{name}"))
    }

    #[test]
    fn cleanup_removes_partial_files_and_enforces_budget() {
        let dir = temp_dir("budget");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("broken.bgra.part"), [0; 4]).unwrap();
        fs::write(dir.join("first.bgra"), [1; 4]).unwrap();
        fs::write(dir.join("second.bgra"), [2; 4]).unwrap();
        fs::write(dir.join("keep.txt"), b"keep").unwrap();

        cleanup_large_image_cache(&dir, 4).unwrap();
        assert!(!dir.join("broken.bgra.part").exists());
        let cache_bytes = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .is_some_and(|value| value == "bgra")
            })
            .map(|entry| entry.metadata().unwrap().len())
            .sum::<u64>();
        assert!(cache_bytes <= 4);
        assert!(dir.join("keep.txt").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn large_image_errors_are_localized_by_category() {
        let disk = LargeImageError::InsufficientDiskSpace {
            required: 10,
            available: 5,
        };
        assert_eq!(
            format_large_image_error(Language::English, &disk),
            "Not enough disk space to prepare this large image"
        );
        assert_eq!(
            format_large_image_error(Language::Chinese, &disk),
            "磁盘空间不足，无法准备这张超大图片"
        );
        assert_eq!(
            format_large_image_error(Language::Chinese, &LargeImageError::InvalidPixelData),
            "无法解码这张超大图片"
        );
    }
}
