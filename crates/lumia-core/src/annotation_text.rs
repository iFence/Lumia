//! Rasterization of annotation text for export copies.
//!
//! `lumia-core` deliberately has no dependency on the UI text stack, so glyphs
//! are rasterized here with `ttf-parser` (already a transitive dependency) over
//! an embedded open-license font subset (Noto Sans SC, Latin + common CJK, OFL).
//! Characters the subset lacks fall back to a hollow placeholder box so text is
//! always visible in exports.

use ttf_parser::{Face, OutlineBuilder};

const FONT_BYTES: &[u8] = include_bytes!("../fonts/NotoSansSC-Regular-subset.ttf");

/// A rasterized line of annotation text: a top-left anchored alpha mask.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextRaster {
    pub width: u32,
    pub height: u32,
    pub alpha: Vec<u8>,
}

enum GlyphOrBox {
    Glyph {
        glyph_id: ttf_parser::GlyphId,
        pen_x: f32,
        advance: f32,
    },
    Box {
        pen_x: f32,
        advance: f32,
    },
}

/// Rasterize a single text line into an alpha mask at the given pixel size.
pub fn rasterize_text_line(text: &str, font_size_px: f32) -> TextRaster {
    let font_size_px = font_size_px.max(1.0);
    let face = match Face::parse(FONT_BYTES, 0) {
        Ok(face) => face,
        Err(_) => return empty_raster(font_size_px),
    };
    let units_per_em = face.units_per_em().max(1) as f32;
    let scale = font_size_px / units_per_em;
    let ascent = face.ascender() as f32;
    let descent = face.descender() as f32;

    let mut pen = 0.0f32;
    let mut glyphs = Vec::with_capacity(text.chars().count());
    for ch in text.chars() {
        match face.glyph_index(ch) {
            Some(glyph_id) => {
                let advance =
                    face.glyph_hor_advance(glyph_id).unwrap_or(units_per_em as u16) as f32;
                glyphs.push(GlyphOrBox::Glyph {
                    glyph_id,
                    pen_x: pen,
                    advance,
                });
                pen += advance;
            }
            None => {
                glyphs.push(GlyphOrBox::Box { pen_x: pen, advance: units_per_em });
                pen += units_per_em;
            }
        }
    }

    let width = (pen * scale).ceil().max(1.0) as u32;
    let height = ((ascent - descent) * scale).ceil().max(1.0) as u32;
    let mut alpha = vec![0u8; (width * height) as usize];

    for glyph in &glyphs {
        match glyph {
            GlyphOrBox::Glyph {
                glyph_id,
                pen_x,
                advance,
            } => {
                let mut builder = ContourBuilder::default();
                if face.outline_glyph(*glyph_id, &mut builder).is_some() {
                    rasterize_contours(&builder.contours, *pen_x, ascent, scale, width, height, &mut alpha);
                } else {
                    rasterize_placeholder_box(*pen_x, *advance, scale, width, height, &mut alpha);
                }
            }
            GlyphOrBox::Box { pen_x, advance } => {
                rasterize_placeholder_box(*pen_x, *advance, scale, width, height, &mut alpha);
            }
        }
    }

    TextRaster { width, height, alpha }
}

/// Blend a text raster into a BGRA8 image at (x, y), top-left anchored.
pub fn blend_text_raster(
    raster: &TextRaster,
    pixels: &mut [u8],
    image_width: u32,
    image_height: u32,
    x: f32,
    y: f32,
    color: u32,
    opacity: f32,
) {
    let red = ((color >> 16) & 0xff) as f32;
    let green = ((color >> 8) & 0xff) as f32;
    let blue = (color & 0xff) as f32;
    for dy in 0..raster.height {
        let iy = (y + dy as f32).round() as i32;
        if iy < 0 || iy >= image_height as i32 {
            continue;
        }
        for dx in 0..raster.width {
            let coverage = raster.alpha[(dy * raster.width + dx) as usize] as f32 / 255.0;
            if coverage <= 0.0 {
                continue;
            }
            let ix = (x + dx as f32).round() as i32;
            if ix < 0 || ix >= image_width as i32 {
                continue;
            }
            blend_pixel(pixels, iy as u32, ix as u32, image_width, color, red, green, blue, coverage * opacity);
        }
    }
}

fn blend_pixel(
    pixels: &mut [u8],
    y: u32,
    x: u32,
    image_width: u32,
    _color: u32,
    red: f32,
    green: f32,
    blue: f32,
    source_alpha: f32,
) {
    let offset = (y as usize * image_width as usize + x as usize) * 4;
    let alpha = source_alpha.clamp(0.0, 1.0);
    let destination_alpha = pixels[offset + 3] as f32 / 255.0;
    let output_alpha = alpha + destination_alpha * (1.0 - alpha);
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

fn empty_raster(font_size_px: f32) -> TextRaster {
    TextRaster {
        width: 1,
        height: font_size_px.ceil().max(1.0) as u32,
        alpha: vec![0; font_size_px.ceil().max(1.0) as usize],
    }
}

#[derive(Default)]
struct ContourBuilder {
    contours: Vec<Vec<(f32, f32)>>,
    current: Vec<(f32, f32)>,
}

impl ContourBuilder {
    fn close_contour(&mut self) {
        if self.current.is_empty() {
            return;
        }
        if self.current.first() != self.current.last() {
            let first = self.current[0];
            self.current.push(first);
        }
        self.contours.push(std::mem::take(&mut self.current));
    }
}

impl OutlineBuilder for ContourBuilder {
    fn move_to(&mut self, x: f32, y: f32) {
        self.close_contour();
        self.current.push((x, y));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.current.push((x, y));
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let start = *self.current.last().unwrap_or(&(0.0, 0.0));
        flatten_quad(start, (x1, y1), (x, y), 0, &mut self.current);
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let start = *self.current.last().unwrap_or(&(0.0, 0.0));
        flatten_cubic(start, (x1, y1), (x2, y2), (x, y), 0, &mut self.current);
    }

    fn close(&mut self) {
        self.close_contour();
    }
}

const FLATNESS_TOLERANCE: f32 = 2.0;
const MAX_SUBDIVISIONS: u8 = 10;

fn flatten_quad(
    p0: (f32, f32),
    p1: (f32, f32),
    p2: (f32, f32),
    depth: u8,
    out: &mut Vec<(f32, f32)>,
) {
    if depth >= MAX_SUBDIVISIONS || distance_point_to_line(p1, p0, p2) <= FLATNESS_TOLERANCE {
        out.push(p2);
        return;
    }
    let m1 = mid(p0, p1);
    let m2 = mid(p1, p2);
    let mid = mid(m1, m2);
    flatten_quad(p0, m1, mid, depth + 1, out);
    flatten_quad(mid, m2, p2, depth + 1, out);
}

fn flatten_cubic(
    p0: (f32, f32),
    p1: (f32, f32),
    p2: (f32, f32),
    p3: (f32, f32),
    depth: u8,
    out: &mut Vec<(f32, f32)>,
) {
    if depth >= MAX_SUBDIVISIONS
        || (distance_point_to_line(p1, p0, p3) <= FLATNESS_TOLERANCE
            && distance_point_to_line(p2, p0, p3) <= FLATNESS_TOLERANCE)
    {
        out.push(p3);
        return;
    }
    let m01 = mid(p0, p1);
    let m12 = mid(p1, p2);
    let m23 = mid(p2, p3);
    let m012 = mid(m01, m12);
    let m123 = mid(m12, m23);
    let m = mid(m012, m123);
    flatten_cubic(p0, m01, m012, m, depth + 1, out);
    flatten_cubic(m, m123, m23, p3, depth + 1, out);
}

fn mid(a: (f32, f32), b: (f32, f32)) -> (f32, f32) {
    ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5)
}

fn distance_point_to_line(point: (f32, f32), a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let length_squared = dx * dx + dy * dy;
    if length_squared <= 1e-6 {
        return ((point.0 - a.0).powi(2) + (point.1 - a.1).powi(2)).sqrt();
    }
    ((dx * (a.1 - point.1) - dy * (a.0 - point.0)) / length_squared.sqrt()).abs()
}

/// Fills `alpha` (width × height) with an anti-aliased even-odd fill of the
/// given glyph contours, converted from font units (origin at the pen, y-up)
/// to image pixels (top-left anchored at `(pen_x, ascent)`).
fn rasterize_contours(
    contours: &[Vec<(f32, f32)>],
    pen_x: f32,
    ascent: f32,
    scale: f32,
    width: u32,
    height: u32,
    alpha: &mut [u8],
) {
    let mut polys: Vec<Vec<(f32, f32)>> = Vec::with_capacity(contours.len());
    let mut bounds = (f32::INFINITY, f32::INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY);
    for contour in contours {
        let mut poly = Vec::with_capacity(contour.len());
        for &(x, y) in contour {
            let px = (pen_x + x) * scale;
            let py = (ascent - y) * scale;
            bounds.0 = bounds.0.min(px);
            bounds.1 = bounds.1.min(py);
            bounds.2 = bounds.2.max(px);
            bounds.3 = bounds.3.max(py);
            poly.push((px, py));
        }
        if poly.len() >= 3 {
            polys.push(poly);
        }
    }
    let x0 = bounds.0.floor().max(0.0) as u32;
    let y0 = bounds.1.floor().max(0.0) as u32;
    let x1 = bounds.2.ceil().min(width as f32) as u32;
    let y1 = bounds.3.ceil().min(height as f32) as u32;
    for py in y0..y1 {
        for px in x0..x1 {
            let mut hits = 0u8;
            for sy in 0..2 {
                for sx in 0..2 {
                    if point_in_polygons(
                        px as f32 + (sx as f32 + 0.5) / 2.0,
                        py as f32 + (sy as f32 + 0.5) / 2.0,
                        &polys,
                    ) {
                        hits += 1;
                    }
                }
            }
            let index = (py * width + px) as usize;
            alpha[index] = alpha[index].max((hits as u32 * 255 / 4) as u8);
        }
    }
}

/// Fills a hollow box (em-width × line-height) for a character the font lacks.
fn rasterize_placeholder_box(
    pen_x: f32,
    advance: f32,
    scale: f32,
    width: u32,
    height: u32,
    alpha: &mut [u8],
) {
    let x0 = pen_x * scale;
    let x1 = (pen_x + advance) * scale;
    let stroke = (advance * scale * 0.08).max(1.0);
    let start_x = x0.floor().max(0.0) as u32;
    let end_x = x1.ceil().min(width as f32) as u32;
    for py in 0..height {
        for px in start_x..end_x {
            let on_edge = (px as f32 - x0).abs() <= stroke
                || (x1 - px as f32).abs() <= stroke
                || (py as f32).abs() <= stroke
                || (height as f32 - 1.0 - py as f32).abs() <= stroke;
            if on_edge {
                alpha[(py * width + px) as usize] = 255;
            }
        }
    }
}

fn point_in_polygons(x: f32, y: f32, polys: &[Vec<(f32, f32)>]) -> bool {
    let mut crossings = 0u32;
    for poly in polys {
        let n = poly.len();
        for i in 0..n {
            let (x1, y1) = poly[i];
            let (x2, y2) = poly[(i + 1) % n];
            if (y1 > y) != (y2 > y) {
                let x_intersection = x1 + (y - y1) * (x2 - x1) / (y2 - y1);
                if x_intersection > x {
                    crossings += 1;
                }
            }
        }
    }
    crossings % 2 == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rasterized_text_has_ink() {
        let raster = rasterize_text_line("A标", 32.0);
        assert!(raster.width > 0);
        assert!(raster.height > 0);
        assert!(raster.alpha.iter().any(|&value| value > 0));
    }

    #[test]
    fn missing_glyphs_draw_a_placeholder_box() {
        // "\u{2FFFF}" is not in the subset, so it must still produce ink.
        let raster = rasterize_text_line("\u{2FFFF}", 24.0);
        assert!(raster.alpha.iter().any(|&value| value > 0));
    }

    #[test]
    fn text_blends_into_bgra() {
        let raster = rasterize_text_line("A", 24.0);
        let mut pixels = vec![0u8; 64 * 64 * 4];
        blend_text_raster(&raster, &mut pixels, 64, 64, 8.0, 8.0, 0xff0000, 1.0);
        assert!(pixels.iter().any(|&value| value > 0));
    }
}
