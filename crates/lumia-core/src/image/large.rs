mod cache;
mod decode;
mod error;
mod mapped;
mod png;
mod tiles;
mod worker;

pub use decode::decode_large_image_preview;
pub use error::LargeImageError;
pub use tiles::{build_large_image_raster, LargeImageRaster};
pub use worker::{large_image_worker_count, PixelBudget};

const DEFAULT_MAX_TEXTURE_EDGE: u32 = 8192;
const DEFAULT_MAX_DECODED_BYTES: u64 = 256 * 1024 * 1024;
const DEFAULT_TILE_SIZE: u32 = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LargeImagePolicy {
    pub max_texture_edge: u32,
    pub max_decoded_bytes: u64,
    pub tile_size: u32,
}

impl Default for LargeImagePolicy {
    fn default() -> Self {
        Self {
            max_texture_edge: DEFAULT_MAX_TEXTURE_EDGE,
            max_decoded_bytes: DEFAULT_MAX_DECODED_BYTES,
            tile_size: DEFAULT_TILE_SIZE,
        }
    }
}

impl LargeImagePolicy {
    pub fn requires_tiling(&self, width: u32, height: u32) -> bool {
        if width > self.max_texture_edge || height > self.max_texture_edge {
            return true;
        }

        checked_bgra8_bytes(width, height).is_none_or(|bytes| bytes > self.max_decoded_bytes)
    }
}

pub fn checked_bgra8_len(width: u32, height: u32) -> Option<usize> {
    usize::try_from(checked_bgra8_bytes(width, height)?).ok()
}

fn checked_bgra8_bytes(width: u32, height: u32) -> Option<u64> {
    u64::from(width)
        .checked_mul(u64::from(height))?
        .checked_mul(4)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImagePixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl ImagePixelRect {
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TileCoordinate {
    pub level: u8,
    pub x: u32,
    pub y: u32,
}

impl TileCoordinate {
    pub const fn new(level: u8, x: u32, y: u32) -> Self {
        Self { level, x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileLevel {
    original_width: u32,
    original_height: u32,
    level: u8,
    divisor: u32,
    width: u32,
    height: u32,
    tile_size: u32,
    columns: u32,
    rows: u32,
}

impl TileLevel {
    pub fn new(width: u32, height: u32, level: u8, tile_size: u32) -> Option<Self> {
        if width == 0 || height == 0 || tile_size == 0 {
            return None;
        }
        let divisor = 1_u32.checked_shl(u32::from(level))?;
        let level_width = div_ceil(width, divisor);
        let level_height = div_ceil(height, divisor);
        Some(Self {
            original_width: width,
            original_height: height,
            level,
            divisor,
            width: level_width,
            height: level_height,
            tile_size,
            columns: div_ceil(level_width, tile_size),
            rows: div_ceil(level_height, tile_size),
        })
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn columns(&self) -> u32 {
        self.columns
    }

    pub const fn rows(&self) -> u32 {
        self.rows
    }

    pub const fn divisor(&self) -> u32 {
        self.divisor
    }

    pub fn tile_rect(&self, coordinate: TileCoordinate) -> Option<ImagePixelRect> {
        if coordinate.level != self.level
            || coordinate.x >= self.columns
            || coordinate.y >= self.rows
        {
            return None;
        }
        let x = coordinate.x.checked_mul(self.tile_size)?;
        let y = coordinate.y.checked_mul(self.tile_size)?;
        Some(ImagePixelRect::new(
            x,
            y,
            self.tile_size.min(self.width - x),
            self.tile_size.min(self.height - y),
        ))
    }

    pub fn source_rect(&self, coordinate: TileCoordinate) -> Option<ImagePixelRect> {
        let rect = self.tile_rect(coordinate)?;
        let x = rect.x.checked_mul(self.divisor)?;
        let y = rect.y.checked_mul(self.divisor)?;
        let right = rect
            .x
            .checked_add(rect.width)?
            .checked_mul(self.divisor)?
            .min(self.original_width);
        let bottom = rect
            .y
            .checked_add(rect.height)?
            .checked_mul(self.divisor)?
            .min(self.original_height);
        Some(ImagePixelRect::new(x, y, right - x, bottom - y))
    }

    pub fn intersecting_tiles(&self, source_rect: ImagePixelRect) -> Vec<TileCoordinate> {
        let left = source_rect.x.min(self.original_width);
        let top = source_rect.y.min(self.original_height);
        let right = u64::from(source_rect.x)
            .saturating_add(u64::from(source_rect.width))
            .min(u64::from(self.original_width)) as u32;
        let bottom = u64::from(source_rect.y)
            .saturating_add(u64::from(source_rect.height))
            .min(u64::from(self.original_height)) as u32;
        if left >= right || top >= bottom {
            return Vec::new();
        }

        let level_left = left / self.divisor;
        let level_top = top / self.divisor;
        let level_right = div_ceil(right, self.divisor);
        let level_bottom = div_ceil(bottom, self.divisor);
        let first_x = level_left / self.tile_size;
        let first_y = level_top / self.tile_size;
        let last_x = (level_right - 1) / self.tile_size;
        let last_y = (level_bottom - 1) / self.tile_size;

        let mut tiles = Vec::new();
        for y in first_y..=last_y {
            for x in first_x..=last_x {
                tiles.push(TileCoordinate::new(self.level, x, y));
            }
        }
        tiles
    }
}

const fn div_ceil(value: u32, divisor: u32) -> u32 {
    value / divisor + if value % divisor == 0 { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_selects_images_that_exceed_texture_or_memory_limits() {
        let policy = LargeImagePolicy::default();

        assert!(!policy.requires_tiling(8192, 8192));
        assert!(policy.requires_tiling(8193, 100));
        assert!(policy.requires_tiling(34752, 11584));
        assert!(policy.requires_tiling(u32::MAX, u32::MAX));
    }

    #[test]
    fn decoded_length_is_checked() {
        assert_eq!(checked_bgra8_len(34752, 11584), Some(1_610_268_672));

        if usize::BITS == 32 {
            assert_eq!(checked_bgra8_len(u32::MAX, u32::MAX), None);
        }
    }

    #[test]
    fn tile_level_rounds_dimensions_up_and_clamps_edge_tiles() {
        let level = TileLevel::new(1000, 700, 0, 512).unwrap();
        assert_eq!(level.width(), 1000);
        assert_eq!(level.height(), 700);
        assert_eq!(level.columns(), 2);
        assert_eq!(level.rows(), 2);
        assert_eq!(
            level.tile_rect(TileCoordinate::new(0, 1, 1)),
            Some(ImagePixelRect::new(512, 512, 488, 188))
        );

        let half = TileLevel::new(1000, 700, 1, 512).unwrap();
        assert_eq!((half.width(), half.height()), (500, 350));
        assert_eq!((half.columns(), half.rows()), (1, 1));
        assert_eq!(
            half.source_rect(TileCoordinate::new(1, 0, 0)),
            Some(ImagePixelRect::new(0, 0, 1000, 700))
        );
    }

    #[test]
    fn intersecting_tiles_are_stable_and_clamped_to_the_image() {
        let level = TileLevel::new(1000, 700, 0, 512).unwrap();
        let visible = ImagePixelRect::new(500, 500, 600, 300);

        assert_eq!(
            level.intersecting_tiles(visible),
            vec![
                TileCoordinate::new(0, 0, 0),
                TileCoordinate::new(0, 1, 0),
                TileCoordinate::new(0, 0, 1),
                TileCoordinate::new(0, 1, 1),
            ]
        );
        assert!(level
            .intersecting_tiles(ImagePixelRect::new(1200, 800, 10, 10))
            .is_empty());
    }

    #[test]
    fn invalid_tile_levels_are_rejected() {
        assert!(TileLevel::new(0, 100, 0, 512).is_none());
        assert!(TileLevel::new(100, 100, 32, 512).is_none());
        assert!(TileLevel::new(100, 100, 0, 0).is_none());
    }
}
