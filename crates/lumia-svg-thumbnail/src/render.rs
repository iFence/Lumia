//! Headless SVG rasterization for Explorer thumbnails.
//!
//! The thumbnail provider runs inside `explorer.exe` / `dllhost.exe`, where
//! there is no GPUI or GPU context. This module turns SVG bytes into a
//! premultiplied RGBA buffer using the pure-Rust `resvg`/`usvg` stack, without
//! touching the filesystem or the network.

use std::sync::Arc;

/// A rasterized SVG frame with premultiplied alpha.
#[allow(dead_code)] // used only on Windows, exercised by cross-platform tests
pub(crate) struct RenderedSvg {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) premultiplied_rgba: Vec<u8>,
}

#[derive(Debug)]
#[allow(dead_code)] // used only on Windows, exercised by cross-platform tests
pub(crate) enum RenderError {
    Parse(String),
    Render(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::Parse(message) => write!(f, "failed to parse SVG: {message}"),
            RenderError::Render(message) => write!(f, "failed to render SVG: {message}"),
        }
    }
}

impl std::error::Error for RenderError {}

/// Rasterize `data` (an SVG document, or a gzip-compressed SVG) to fit within a
/// `target`-pixel square, preserving the SVG's aspect ratio.
#[allow(dead_code)] // used only on Windows, exercised by cross-platform tests
pub(crate) fn svg_bytes_to_rgba(data: &[u8], target: u32) -> Result<RenderedSvg, RenderError> {
    let target = target.clamp(16, 512) as f32;

    let mut font_database = usvg::fontdb::Database::new();
    font_database.load_system_fonts();

    let mut options = usvg::Options::default();
    options.fontdb = Arc::new(font_database);
    // Never load external resources referenced by the SVG: the provider runs
    // in a shell process and must not perform network or arbitrary file access.
    options.resources_dir = None;

    let tree = usvg::Tree::from_data(data, &options)
        .map_err(|error| RenderError::Parse(error.to_string()))?;

    let size = tree.size();
    let (svg_width, svg_height) = (size.width(), size.height());
    if !(svg_width > 0.0 && svg_height > 0.0) {
        return Err(RenderError::Parse("SVG has empty bounds".into()));
    }

    // Contain the thumbnail within the requested square box, preserving aspect
    // ratio. This also keeps the output buffer bounded for extreme aspect
    // ratios or large intrinsic sizes.
    let scale = (target / svg_width).min(target / svg_height);
    let out_width = (svg_width * scale).round().max(1.0) as u32;
    let out_height = (svg_height * scale).round().max(1.0) as u32;

    let mut pixmap = resvg::tiny_skia::Pixmap::new(out_width, out_height)
        .ok_or_else(|| RenderError::Render("could not allocate output buffer".into()))?;
    let transform = resvg::tiny_skia::Transform::from_scale(scale, scale);
    resvg::render(&tree, transform, &mut pixmap.as_mut());

    Ok(RenderedSvg {
        width: out_width,
        height: out_height,
        premultiplied_rgba: pixmap.data().to_vec(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED_RECT: &str = r##"
        <svg xmlns="http://www.w3.org/2000/svg" width="100" height="50">
            <rect x="0" y="0" width="100" height="50" fill="#ff0000"/>
        </svg>"##;

    #[test]
    fn renders_opaque_red_rect_at_requested_size() {
        let rendered = svg_bytes_to_rgba(RED_RECT.as_bytes(), 256).unwrap();
        assert_eq!((rendered.width, rendered.height), (256, 128));
        let pixel = &rendered.premultiplied_rgba[..4];
        assert_eq!([pixel[0], pixel[1], pixel[2], pixel[3]], [255, 0, 0, 255]);
    }

    #[test]
    fn clamps_oversized_requests_to_512() {
        let rendered = svg_bytes_to_rgba(RED_RECT.as_bytes(), 8192).unwrap();
        assert_eq!((rendered.width, rendered.height), (512, 256));
    }

    #[test]
    fn contains_wide_svg_within_square_box() {
        let wide = r##"
            <svg xmlns="http://www.w3.org/2000/svg" width="1000" height="100">
                <rect width="1000" height="100" fill="#00ff00"/>
            </svg>"##;
        let rendered = svg_bytes_to_rgba(wide.as_bytes(), 256).unwrap();
        assert_eq!((rendered.width, rendered.height), (256, 26));
    }

    #[test]
    fn bounds_extreme_vertical_svg() {
        let tall = r##"
            <svg xmlns="http://www.w3.org/2000/svg" width="10" height="10000">
                <rect width="10" height="10000" fill="#0000ff"/>
            </svg>"##;
        let rendered = svg_bytes_to_rgba(tall.as_bytes(), 256).unwrap();
        assert_eq!((rendered.width, rendered.height), (1, 256));
    }

    #[test]
    fn rejects_invalid_svg() {
        assert!(svg_bytes_to_rgba(b"not an svg", 64).is_err());
    }

    #[test]
    fn accepts_gzip_compressed_svg() {
        use std::io::Write;

        let mut gzip = flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::default(),
        );
        gzip.write_all(RED_RECT.as_bytes()).unwrap();
        let compressed = gzip.finish().unwrap();

        let rendered = svg_bytes_to_rgba(&compressed, 128).unwrap();
        assert_eq!((rendered.width, rendered.height), (128, 64));
    }
}
