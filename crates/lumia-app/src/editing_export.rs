use std::path::Path;

use gpui::Context;
use lumia_core::{
    apply_image_edit, export_decoded_image, load_decoded_image_from_path_with_policy,
    rotate_decoded_image, DecodeCancellation, DecodePolicy, ImageEditOperation, ImageEditPolicy,
    ImageExportFormat,
};

use crate::app::LumiaApp;
use crate::editing::EditMode;

impl LumiaApp {
    pub(crate) fn export_edit_copy(&mut self, cx: &mut Context<Self>) {
        if self.editing.exporting {
            return;
        }
        let Some(mode) = self.editing.mode else {
            return;
        };
        let Some(source_path) = self.image_path().map(Path::to_path_buf) else {
            return;
        };
        let operation = match mode {
            EditMode::Crop => ImageEditOperation::Crop(self.editing.crop_rect),
            EditMode::Resize => {
                let Some((width, height)) = self.current_resize_values(cx) else {
                    self.editing.feedback = Some(Err("Invalid output dimensions".into()));
                    cx.notify();
                    return;
                };
                if !self.resize_is_valid(cx) {
                    self.editing.feedback =
                        Some(Err("Output image exceeds the 96 MiB limit".into()));
                    cx.notify();
                    return;
                }
                ImageEditOperation::Resize { width, height }
            }
        };
        let rotation = self.editing.rotation_quarter_turns;
        let generation = self.editing.generation;
        let default_extension = default_export_format(&source_path).extension();
        let default_name = default_export_name(&source_path, mode, default_extension);
        let default_directory = source_path.parent().map(Path::to_path_buf);
        self.editing.exporting = true;
        self.editing.feedback = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let picked = cx
                .background_executor()
                .spawn(async move {
                    let mut dialog = rfd::FileDialog::new()
                        .add_filter("PNG", &["png"])
                        .add_filter("JPEG", &["jpg", "jpeg"])
                        .add_filter("WebP", &["webp"])
                        .set_file_name(&default_name);
                    if let Some(directory) = default_directory {
                        dialog = dialog.set_directory(directory);
                    }
                    dialog.save_file()
                })
                .await;
            let Some(mut output_path) = picked else {
                let _ = this.update(cx, |this, cx| {
                    if this.editing.generation == generation {
                        this.editing.exporting = false;
                        cx.notify();
                    }
                });
                return;
            };
            if output_path.extension().is_none() {
                output_path.set_extension(default_extension);
            }
            let format = output_path
                .extension()
                .and_then(|extension| extension.to_str())
                .and_then(ImageExportFormat::from_extension);
            let result = match format {
                Some(format) => {
                    let task_source = source_path.clone();
                    let task_output = output_path.clone();
                    cx.background_executor()
                        .spawn(async move {
                            let decoded = load_decoded_image_from_path_with_policy(
                                &task_source,
                                DecodePolicy::default(),
                                &DecodeCancellation::default(),
                            )
                            .map_err(|error| error.to_string())?;
                            let rotated = rotate_decoded_image(&decoded, rotation)
                                .map_err(|error| error.to_string())?;
                            let edited =
                                apply_image_edit(&rotated, operation, ImageEditPolicy::default())
                                    .map_err(|error| error.to_string())?;
                            export_decoded_image(&edited, &task_output, format)
                                .map_err(|error| error.to_string())?;
                            Ok::<_, String>(task_output)
                        })
                        .await
                }
                None => Err("Choose a PNG, JPEG, or WebP file name".into()),
            };
            let _ = this.update(cx, |this, cx| {
                if this.editing.generation == generation {
                    this.editing.exporting = false;
                    this.editing.feedback = Some(result);
                    cx.notify();
                }
            });
        })
        .detach();
    }
}

fn default_export_format(path: &Path) -> ImageExportFormat {
    path.extension()
        .and_then(|extension| extension.to_str())
        .and_then(ImageExportFormat::from_extension)
        .unwrap_or(ImageExportFormat::Png)
}

fn default_export_name(path: &Path, mode: EditMode, extension: &str) -> String {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("image");
    let suffix = match mode {
        EditMode::Crop => "cropped",
        EditMode::Resize => "resized",
    };
    format!("{stem}-{suffix}.{extension}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_names_describe_the_operation() {
        assert_eq!(
            default_export_name(Path::new("/tmp/photo.jpg"), EditMode::Crop, "jpg"),
            "photo-cropped.jpg"
        );
        assert_eq!(
            default_export_name(Path::new("photo.tiff"), EditMode::Resize, "png"),
            "photo-resized.png"
        );
    }
}
