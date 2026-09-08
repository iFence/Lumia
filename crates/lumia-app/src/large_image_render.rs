use gpui::{
    canvas, div, img, point, px, size, AnyElement, Bounds, ContentMask, InteractiveElement,
    IntoElement, ObjectFit, ParentElement, Styled, StyledImage, Window,
};
use lumia_core::{ImagePixelRect, TileCoordinate, TileLevel};

use crate::{
    app::LumiaApp,
    large_image::{LARGE_IMAGE_TILE_GUTTER, LARGE_IMAGE_TILE_SIZE},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LargeImageViewGeometry {
    pub(crate) level: u8,
    pub(crate) visible_source: ImagePixelRect,
    pub(crate) visible_tiles: Vec<TileCoordinate>,
    pub(crate) prefetch_tiles: Vec<TileCoordinate>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TileDisplayRect {
    left: f32,
    top: f32,
    width: f32,
    height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct LargeImageTileLayout {
    clip: TileDisplayRect,
    image: TileDisplayRect,
}

impl LargeImageTileLayout {
    fn calculate(level: &TileLevel, coordinate: TileCoordinate, scale: f32) -> Option<Self> {
        if !scale.is_finite() || scale <= 0.0 {
            return None;
        }
        let clip = scaled_rect(level.source_rect(coordinate)?, scale)?;
        let padded = scaled_rect(
            level.tile_rect_with_gutter(coordinate, LARGE_IMAGE_TILE_GUTTER)?,
            scale * level.divisor() as f32,
        )?;
        Some(Self {
            clip,
            image: TileDisplayRect {
                left: padded.left - clip.left,
                top: padded.top - clip.top,
                width: padded.width,
                height: padded.height,
            },
        })
    }
}

fn scaled_rect(rect: ImagePixelRect, scale: f32) -> Option<TileDisplayRect> {
    let right = rect.x.checked_add(rect.width)?;
    let bottom = rect.y.checked_add(rect.height)?;
    let left = rect.x as f32 * scale;
    let top = rect.y as f32 * scale;
    let right = right as f32 * scale;
    let bottom = bottom as f32 * scale;
    Some(TileDisplayRect {
        left,
        top,
        width: right - left,
        height: bottom - top,
    })
}

impl LargeImageViewGeometry {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn calculate(
        image_width: u32,
        image_height: u32,
        viewport_width: f32,
        viewport_height: f32,
        scale: f32,
        pan_x: f32,
        pan_y: f32,
        rotation_quarter_turns: u8,
    ) -> Option<Self> {
        if image_width == 0
            || image_height == 0
            || !viewport_width.is_finite()
            || !viewport_height.is_finite()
            || !scale.is_finite()
            || viewport_width <= 0.0
            || viewport_height <= 0.0
            || scale <= 0.0
            || rotation_quarter_turns % 4 != 0
        {
            return None;
        }
        let level = level_for_scale(scale);
        let tile_level = TileLevel::new(image_width, image_height, level, LARGE_IMAGE_TILE_SIZE)?;
        let display_width = image_width as f32 * scale;
        let display_height = image_height as f32 * scale;
        let image_left = (viewport_width - display_width) / 2.0 + pan_x;
        let image_top = (viewport_height - display_height) / 2.0 + pan_y;
        let left = ((-image_left / scale).floor().max(0.0) as u32).min(image_width);
        let top = ((-image_top / scale).floor().max(0.0) as u32).min(image_height);
        let right =
            (((viewport_width - image_left) / scale).ceil().max(0.0) as u32).min(image_width);
        let bottom =
            (((viewport_height - image_top) / scale).ceil().max(0.0) as u32).min(image_height);
        if left >= right || top >= bottom {
            return None;
        }
        let visible_source = ImagePixelRect::new(left, top, right - left, bottom - top);
        let visible_tiles = tile_level.intersecting_tiles(visible_source);
        let margin = LARGE_IMAGE_TILE_SIZE.saturating_mul(tile_level.divisor());
        let prefetch_left = left.saturating_sub(margin);
        let prefetch_top = top.saturating_sub(margin);
        let prefetch_right = right.saturating_add(margin).min(image_width);
        let prefetch_bottom = bottom.saturating_add(margin).min(image_height);
        let prefetch_source = ImagePixelRect::new(
            prefetch_left,
            prefetch_top,
            prefetch_right - prefetch_left,
            prefetch_bottom - prefetch_top,
        );
        let prefetch_tiles = tile_level
            .intersecting_tiles(prefetch_source)
            .into_iter()
            .filter(|coordinate| !visible_tiles.contains(coordinate))
            .collect();
        Some(Self {
            level,
            visible_source,
            visible_tiles,
            prefetch_tiles,
        })
    }
}

fn level_for_scale(scale: f32) -> u8 {
    if scale >= 1.0 {
        return 0;
    }
    let target = (1.0 / scale).floor().max(1.0) as u32;
    (31 - target.leading_zeros()) as u8
}

impl LumiaApp {
    /// Tile scheduling and painting must use the same pane geometry.
    pub(crate) fn large_image_viewport(&self, window: &Window) -> Option<(f32, f32, f32)> {
        if let Some(state) = self.comparison.as_ref() {
            let bounds = state.left_pane_bounds?;
            let dimensions = self.viewer.display_dimensions()?;
            let viewport = self.viewer.viewport();
            let scale = if viewport.fit_mode == lumia_core::FitMode::FitToWindow {
                crate::display_resample::pane_fit_scale(bounds.size, dimensions)
            } else {
                viewport.zoom
            };
            Some((
                f32::from(bounds.size.width),
                f32::from(bounds.size.height),
                scale,
            ))
        } else {
            let (width, height) = self.viewer_available_size(window);
            Some((width, height, self.image_display_scale(window)?))
        }
    }

    pub(crate) fn render_large_image_content(&self, window: &Window) -> Option<AnyElement> {
        let path = self.image_path()?;
        if !self.large_image.is_active(path) || !self.large_image.is_preview_ready() {
            return None;
        }
        let prepared = self
            .loads
            .display_image(self.viewer.rotation_quarter_turns())?;
        let (image_width, image_height) = self.viewer.display_dimensions()?;
        let (viewport_width, viewport_height, scale) = self.large_image_viewport(window)?;
        let (display_width, display_height) =
            (image_width as f32 * scale, image_height as f32 * scale);
        let mut content = div()
            .id("large-image-content")
            .flex_shrink_0()
            .relative()
            .w(px(display_width))
            .h(px(display_height))
            .child(
                img(prepared.render_image())
                    .absolute()
                    .size_full()
                    .object_fit(ObjectFit::Fill),
            );
        if self.viewer.rotation_quarter_turns() != 0 {
            return Some(content.into_any_element());
        }

        let geometry = LargeImageViewGeometry::calculate(
            image_width,
            image_height,
            viewport_width,
            viewport_height,
            scale,
            self.viewer.viewport().pan_x,
            self.viewer.viewport().pan_y,
            0,
        )?;
        let level = TileLevel::new(
            image_width,
            image_height,
            geometry.level,
            LARGE_IMAGE_TILE_SIZE,
        )?;
        let tiles: Vec<_> = geometry
            .visible_tiles
            .into_iter()
            .filter_map(|coordinate| {
                let tile = self.large_image.tile(&coordinate)?;
                let layout = LargeImageTileLayout::calculate(&level, coordinate, scale)?;
                Some((tile.render_image(), layout))
            })
            .collect();
        content = content.child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    // Derive every boundary from one origin, bypassing independent
                    // flex-layout rounding of each tile and its gutter image.
                    for (tile, layout) in tiles {
                        let clip = Bounds::new(
                            bounds.origin + point(px(layout.clip.left), px(layout.clip.top)),
                            size(px(layout.clip.width), px(layout.clip.height)),
                        );
                        let image = Bounds::new(
                            clip.origin + point(px(layout.image.left), px(layout.image.top)),
                            size(px(layout.image.width), px(layout.image.height)),
                        );
                        window.with_content_mask(Some(ContentMask { bounds: clip }), |window| {
                            let _ = window.paint_image(
                                image,
                                image,
                                Default::default(),
                                tile,
                                0,
                                false,
                            );
                        });
                    }
                },
            )
            .absolute()
            .size_full(),
        );
        Some(content.into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lumia_core::{ImagePixelRect, TileCoordinate};

    #[test]
    fn fit_scale_uses_a_downsampled_level_and_full_source_bounds() {
        let geometry =
            LargeImageViewGeometry::calculate(10_000, 5_000, 1_000.0, 800.0, 0.1, 0.0, 0.0, 0)
                .unwrap();
        assert_eq!(geometry.level, 3);
        assert_eq!(
            geometry.visible_source,
            ImagePixelRect::new(0, 0, 10_000, 5_000)
        );
        assert!(geometry
            .visible_tiles
            .contains(&TileCoordinate::new(3, 0, 0)));
    }

    #[test]
    fn actual_size_maps_centered_viewport_to_source_pixels() {
        let geometry =
            LargeImageViewGeometry::calculate(10_000, 5_000, 1_000.0, 800.0, 1.0, 0.0, 0.0, 0)
                .unwrap();
        assert_eq!(geometry.level, 0);
        assert_eq!(
            geometry.visible_source,
            ImagePixelRect::new(4_500, 2_100, 1_000, 800)
        );
    }

    #[test]
    fn gutter_layout_clips_images_on_shared_non_integer_scale_boundaries() {
        let level = TileLevel::new(1400, 1100, 0, LARGE_IMAGE_TILE_SIZE).unwrap();
        let left =
            LargeImageTileLayout::calculate(&level, TileCoordinate::new(0, 0, 0), 1.25).unwrap();
        let right =
            LargeImageTileLayout::calculate(&level, TileCoordinate::new(0, 1, 0), 1.25).unwrap();

        assert_eq!(left.clip.left + left.clip.width, right.clip.left);
        assert_eq!(left.clip.width, 640.0);
        assert_eq!(left.image.left, 0.0);
        assert_eq!(left.image.width, 641.25);
        assert_eq!(right.image.left, -1.25);
        assert_eq!(right.image.width, 642.5);
        assert!(left.image.width > left.clip.width);
        assert!(right.image.left < 0.0);
    }

    #[test]
    fn odd_sized_edge_tiles_keep_the_same_texel_scale() {
        let level = TileLevel::new(2051, 1031, 1, LARGE_IMAGE_TILE_SIZE).unwrap();
        let coordinate = TileCoordinate::new(1, 2, 0);
        let layout = LargeImageTileLayout::calculate(&level, coordinate, 0.73).unwrap();
        let pixels = level
            .tile_rect_with_gutter(coordinate, LARGE_IMAGE_TILE_GUTTER)
            .unwrap();
        assert!((layout.image.width - pixels.width as f32 * 2.0 * 0.73).abs() < 0.001);
        // The last texel extends past the source edge; clip it instead of
        // squeezing the entire edge tile into a different sampling scale.
        assert!(layout.image.left + layout.image.width > layout.clip.width);
    }

    #[test]
    fn fractional_scale_tiles_share_both_axes() {
        let level = TileLevel::new(20000, 15001, 3, LARGE_IMAGE_TILE_SIZE).unwrap();
        for scale in [0.137, 0.333, 0.731] {
            let a = LargeImageTileLayout::calculate(&level, TileCoordinate::new(3, 0, 0), scale)
                .unwrap();
            let b = LargeImageTileLayout::calculate(&level, TileCoordinate::new(3, 1, 0), scale)
                .unwrap();
            let c = LargeImageTileLayout::calculate(&level, TileCoordinate::new(3, 0, 1), scale)
                .unwrap();
            assert_eq!(a.clip.left + a.clip.width, b.clip.left);
            assert_eq!(a.clip.top + a.clip.height, c.clip.top);
        }
    }

    #[test]
    fn pan_moves_visible_source_and_rotation_disables_tiles() {
        let centered =
            LargeImageViewGeometry::calculate(10_000, 5_000, 1_000.0, 800.0, 1.0, 0.0, 0.0, 0)
                .unwrap();
        let panned =
            LargeImageViewGeometry::calculate(10_000, 5_000, 1_000.0, 800.0, 1.0, 100.0, 0.0, 0)
                .unwrap();
        assert!(panned.visible_source.x < centered.visible_source.x);
        assert!(
            LargeImageViewGeometry::calculate(10_000, 5_000, 1_000.0, 800.0, 1.0, 0.0, 0.0, 1,)
                .is_none()
        );
    }
}
