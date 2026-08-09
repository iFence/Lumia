use std::path::{Path, PathBuf};

use gpui::{Context, Window};
use lumia_core::{
    blend_text_raster, export_decoded_image, rasterize_text_line, Annotation, DecodedImage,
    ImageExportFormat,
};

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
    annotations: &[Annotation],
    output_path: PathBuf,
    invalid_format_error: &str,
) -> Result<Option<PathBuf>, String> {
    for annotation in annotations {
        draw_annotation(&mut image, annotation);
    }
    let format = output_path
        .extension()
        .and_then(|extension| extension.to_str())
        .and_then(ImageExportFormat::from_extension)
        .ok_or_else(|| invalid_format_error.to_string())?;
    export_decoded_image(&image, &output_path, format).map_err(|error| error.to_string())?;
    Ok(Some(output_path))
}

fn draw_annotation(image: &mut DecodedImage, annotation: &Annotation) {
    match annotation {
        Annotation::Text {
            text,
            x,
            y,
            font_size,
            color,
            opacity,
        } => {
            let raster = rasterize_text_line(text, *font_size);
            blend_text_raster(
                &raster,
                &mut image.pixels_bgra8,
                image.width,
                image.height,
                *x,
                *y,
                *color,
                *opacity,
            );
        }
        Annotation::Rectangle {
            x,
            y,
            width,
            height,
            stroke_width,
            color,
            opacity,
        } => draw_rectangle(
            image,
            *x,
            *y,
            *width,
            *height,
            *stroke_width,
            *color,
            *opacity,
        ),
        Annotation::Step {
            number,
            x,
            y,
            size,
            color,
            opacity,
        } => draw_step(image, *number, *x, *y, *size, *color, *opacity),
    }
}

fn draw_rectangle(
    image: &mut DecodedImage,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    stroke_width: f32,
    color: u32,
    opacity: f32,
) {
    let stroke = stroke_width.max(1.0);
    let min_x = x.floor().max(0.0) as u32;
    let max_x = (x + width).ceil().min(image.width as f32) as u32;
    let min_y = y.floor().max(0.0) as u32;
    let max_y = (y + height).ceil().min(image.height as f32) as u32;
    for py in min_y..max_y {
        for px in min_x..max_x {
            let near_left = (px as f32 - x).abs() <= stroke;
            let near_right = ((x + width) - px as f32).abs() <= stroke;
            let near_top = (py as f32 - y).abs() <= stroke;
            let near_bottom = ((y + height) - py as f32).abs() <= stroke;
            if near_left || near_right || near_top || near_bottom {
                blend_bgra(
                    &mut image.pixels_bgra8,
                    image.width,
                    px,
                    py,
                    color,
                    opacity,
                );
            }
        }
    }
}

fn draw_step(
    image: &mut DecodedImage,
    number: u32,
    x: f32,
    y: f32,
    size: f32,
    color: u32,
    opacity: f32,
) {
    let radius = (size / 2.0).max(2.0);
    let min_x = (x - radius).floor().max(0.0) as u32;
    let max_x = (x + radius).ceil().min(image.width as f32) as u32;
    let min_y = (y - radius).floor().max(0.0) as u32;
    let max_y = (y + radius).ceil().min(image.height as f32) as u32;
    for py in min_y..=max_y {
        for px in min_x..=max_x {
            let dx = px as f32 - x;
            let dy = py as f32 - y;
            if dx * dx + dy * dy <= radius * radius {
                blend_bgra(&mut image.pixels_bgra8, image.width, px, py, color, opacity);
            }
        }
    }

    let raster = rasterize_text_line(&number.to_string(), size * 0.6);
    blend_text_raster(
        &raster,
        &mut image.pixels_bgra8,
        image.width,
        image.height,
        x - raster.width as f32 / 2.0,
        y - raster.height as f32 / 2.0,
        0xffffff,
        opacity,
    );
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

    fn transparent_image(width: u32, height: u32) -> DecodedImage {
        DecodedImage {
            pixels_bgra8: vec![0u8; (width * height * 4) as usize],
            width,
            height,
        }
    }

    #[test]
    fn marker_export_names_are_non_destructive() {
        assert_eq!(
            annotation_export_name(Path::new("/tmp/photo.jpg")),
            "photo-annotated.png"
        );
    }

    #[test]
    fn rectangle_export_draws_only_the_outline() {
        let mut image = transparent_image(100, 100);
        draw_rectangle(&mut image, 20.0, 20.0, 60.0, 60.0, 4.0, 0xff0000, 1.0);
        let red_at = |x: u32, y: u32| image.pixels_bgra8[(y * 100 + x) as usize * 4 + 2];
        assert!(red_at(22, 22) > 0, "top edge should be drawn");
        assert!(red_at(50, 22) > 0, "top edge should be drawn");
        assert!(red_at(78, 50) > 0, "right edge should be drawn");
        assert_eq!(red_at(50, 50), 0, "interior should be empty");
    }

    #[test]
    fn step_badge_includes_its_center_and_number() {
        let mut image = transparent_image(100, 100);
        draw_step(&mut image, 3, 50.0, 50.0, 24.0, 0xff0000, 1.0);
        let red_at = |x: u32, y: u32| image.pixels_bgra8[(y * 100 + x) as usize * 4 + 2];
        assert!(red_at(50, 50) > 0, "badge center should be filled");
        let white_blue_at = |x: u32, y: u32| image.pixels_bgra8[(y * 100 + x) as usize * 4];
        assert!(
            (0..100).any(|y| (0..100).any(|x| white_blue_at(x, y) > 0)),
            "the white number glyphs should add ink"
        );
    }

    #[test]
    fn marker_blending_preserves_partial_alpha() {
        let mut pixels = vec![0, 0, 0, 0];
        blend_bgra(&mut pixels, 1, 0, 0, 0xff0000, 0.5);
        assert_eq!(&pixels[..3], &[0, 0, 255]);
        assert_eq!(pixels[3], 128);
    }
}
