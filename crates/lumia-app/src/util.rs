use chrono::{DateTime, Local};
use gpui::{div, rgb, InteractiveElement, IntoElement, ParentElement, Styled};
use lumia_core::ImageLoadError;
use std::time::UNIX_EPOCH;

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
        | ImageLoadError::Io { .. } => {
            "Could not read image metadata".to_string()
        }
    }
}
