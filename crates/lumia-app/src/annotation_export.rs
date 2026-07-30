use std::path::{Path, PathBuf};

use gpui::{Context, Window};
use lumia_core::{export_decoded_image, DecodedImage, IconAnnotation, ImageExportFormat};

use crate::app::LumiaApp;
use crate::i18n::{tr, TextKey};

impl LumiaApp {
    pub(crate) fn plugin_canvas_available(&self) -> bool {
        if self.image_path().is_some_and(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(lumia_core::is_raw_image_extension)
        }) {
            return false;
        }

        let Some(source_dimensions) = self.viewer.display_dimensions() else {
            return false;
        };
        self.loads
            .display_image(self.viewer.rotation_quarter_turns())
            .is_some_and(|image| {
                image.dimensions() == source_dimensions && image.pixels_bgra8().is_some()
            })
    }

    pub(crate) fn export_annotation_copy(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let Some(source_path) = self.image_path().map(Path::to_path_buf) else {
            return;
        };
        let Some(prepared) = self
            .loads
            .display_image(self.viewer.rotation_quarter_turns())
        else {
            self.plugins.feedback =
                Some(tr(self.settings.language, TextKey::AnnotationNotReady).into());
            cx.notify();
            return;
        };
        let Some(pixels) = prepared.pixels_bgra8().map(ToOwned::to_owned) else {
            self.plugins.feedback =
                Some(tr(self.settings.language, TextKey::AnnotationPixelsUnavailable).into());
            cx.notify();
            return;
        };
        let (width, height) = prepared.dimensions();
        let annotations = self.annotations.items().to_vec();
        let default_name = annotation_export_name(&source_path);
        let default_directory = source_path.parent().map(Path::to_path_buf);
        let invalid_format_error =
            tr(self.settings.language, TextKey::AnnotationChooseFormat).to_string();
        let exported_label = tr(self.settings.language, TextKey::AnnotationExported);
        let generation = self.plugins.generation;
        self.plugins.feedback = None;
        if let Some(active) = self.plugins.active.as_mut() {
            active.busy = true;
        }
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
            let result = if let Some(mut output_path) = picked {
                if output_path.extension().is_none() {
                    output_path.set_extension("png");
                }
                export_annotations(
                    DecodedImage {
                        pixels_bgra8: pixels,
                        width,
                        height,
                    },
                    &annotations,
                    output_path,
                    &invalid_format_error,
                )
            } else {
                Ok(None)
            };
            let _ = this.update(cx, |this, cx| {
                if this.plugins.generation != generation {
                    return;
                }
                if let Some(active) = this.plugins.active.as_mut() {
                    active.busy = false;
                }
                match result {
                    Ok(Some(path)) => {
                        this.plugins.feedback =
                            Some(format!("{exported_label} {}", path.display()));
                    }
                    Ok(None) => {}
                    Err(error) => this.plugins.feedback = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }
}

fn export_annotations(
    mut image: DecodedImage,
    annotations: &[IconAnnotation],
    output_path: PathBuf,
    invalid_format_error: &str,
) -> Result<Option<PathBuf>, String> {
    for annotation in annotations {
        draw_marker(&mut image, annotation);
    }
    let format = output_path
        .extension()
        .and_then(|extension| extension.to_str())
        .and_then(ImageExportFormat::from_extension)
        .ok_or_else(|| invalid_format_error.to_string())?;
    export_decoded_image(&image, &output_path, format).map_err(|error| error.to_string())?;
    Ok(Some(output_path))
}

fn draw_marker(image: &mut DecodedImage, annotation: &IconAnnotation) {
    let radius = (annotation.size / 2.0).max(2.0);
    let min_x = (annotation.x - radius).floor().max(0.0) as u32;
    let max_x = (annotation.x + radius)
        .ceil()
        .min(image.width.saturating_sub(1) as f32) as u32;
    let min_y = (annotation.y - radius).floor().max(0.0) as u32;
    let max_y = (annotation.y + radius)
        .ceil()
        .min(image.height.saturating_sub(1) as f32) as u32;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let local_x = (x as f32 - annotation.x) / radius;
            let local_y = (y as f32 - annotation.y) / radius;
            if marker_contains(&annotation.asset_id, local_x, local_y) {
                blend_bgra(
                    &mut image.pixels_bgra8,
                    image.width,
                    x,
                    y,
                    annotation.color,
                    annotation.opacity,
                );
            }
        }
    }
}

fn marker_contains(asset_id: &str, x: f32, y: f32) -> bool {
    match asset_id {
        "star" => {
            let angle = y.atan2(x);
            let distance = (x * x + y * y).sqrt();
            let boundary = if (angle * 5.0).cos() >= 0.0 {
                1.0
            } else {
                0.45
            };
            distance <= boundary
        }
        "check" => {
            let circle = x * x + y * y <= 1.0;
            let first = (y - (x + 0.2)).abs() < 0.14 && x < 0.0;
            let second = (y + 0.55 * x - 0.15).abs() < 0.14 && x >= -0.2;
            circle && !(first || second)
        }
        _ => {
            let head = x * x + (y + 0.2) * (y + 0.2) <= 0.65;
            let tail = y >= 0.1 && y <= 1.0 && x.abs() <= (1.0 - y) * 0.55;
            head || tail
        }
    }
}

fn blend_bgra(pixels: &mut [u8], width: u32, x: u32, y: u32, color: u32, opacity: f32) {
    let offset = (y as usize * width as usize + x as usize) * 4;
    let alpha = opacity.clamp(0.0, 1.0);
    let destination_alpha = pixels[offset + 3] as f32 / 255.0;
    let output_alpha = alpha + destination_alpha * (1.0 - alpha);
    let red = ((color >> 16) & 0xff) as f32;
    let green = ((color >> 8) & 0xff) as f32;
    let blue = (color & 0xff) as f32;
    let blend = |source: f32, destination: u8| {
        if output_alpha <= f32::EPSILON {
            0
        } else {
            ((source * alpha + destination as f32 * destination_alpha * (1.0 - alpha))
                / output_alpha)
                .round()
                .clamp(0.0, 255.0) as u8
        }
    };
    pixels[offset] = blend(blue, pixels[offset]);
    pixels[offset + 1] = blend(green, pixels[offset + 1]);
    pixels[offset + 2] = blend(red, pixels[offset + 2]);
    pixels[offset + 3] = (output_alpha * 255.0).round().clamp(0.0, 255.0) as u8;
}

fn annotation_export_name(path: &Path) -> String {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("image");
    format!("{stem}-annotated.png")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_export_names_are_non_destructive() {
        assert_eq!(
            annotation_export_name(Path::new("/tmp/photo.jpg")),
            "photo-annotated.png"
        );
    }

    #[test]
    fn marker_shapes_include_their_center() {
        assert!(marker_contains("pin", 0.0, 0.0));
        assert!(marker_contains("star", 0.0, 0.0));
        assert!(marker_contains("check", 0.0, 0.0));
    }

    #[test]
    fn marker_blending_preserves_partial_alpha() {
        let mut pixels = vec![0, 0, 0, 0];
        blend_bgra(&mut pixels, 1, 0, 0, 0xff0000, 0.5);
        assert_eq!(&pixels[..3], &[0, 0, 255]);
        assert_eq!(pixels[3], 128);
    }
}
