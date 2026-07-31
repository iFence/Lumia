use std::{
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    sync::Arc,
};

use image::ImageFormat;
use png::{BitDepth, Decoder, Transformations};

use super::{
    LargeImageError, TileCoordinate, TileLevel,
    cache::{RasterCacheKey, RasterCacheReader, RasterCacheWriter, RasterLayout},
    mapped::write_mapped_bgra_raster,
    png::{png_channels, png_pixel},
};
use crate::{DecodeCancellation, DecodedImage};

#[derive(Clone)]
pub struct LargeImageRaster {
    path: PathBuf,
    layout: RasterLayout,
    reader: Arc<RasterCacheReader>,
}

impl LargeImageRaster {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn dimensions(&self) -> (u32, u32) {
        (self.layout.width(), self.layout.height())
    }

    pub fn decode_tile(
        &self,
        coordinate: TileCoordinate,
        tile_size: u32,
        cancellation: &DecodeCancellation,
    ) -> Result<DecodedImage, LargeImageError> {
        self.decode_tile_with_gutter(coordinate, tile_size, 0, cancellation)
    }

    pub fn decode_tile_with_gutter(
        &self,
        coordinate: TileCoordinate,
        tile_size: u32,
        gutter: u32,
        cancellation: &DecodeCancellation,
    ) -> Result<DecodedImage, LargeImageError> {
        if cancellation.is_cancelled() {
            return Err(LargeImageError::Cancelled);
        }
        let level = TileLevel::new(
            self.layout.width(),
            self.layout.height(),
            coordinate.level,
            tile_size,
        )
        .ok_or(LargeImageError::InvalidTileCoordinate)?;
        let tile = level
            .tile_rect_with_gutter(coordinate, gutter)
            .ok_or(LargeImageError::InvalidTileCoordinate)?;
        let len = usize::try_from(
            u64::from(tile.width)
                .checked_mul(u64::from(tile.height))
                .and_then(|pixels| pixels.checked_mul(4))
                .ok_or(LargeImageError::SizeOverflow)?,
        )
        .map_err(|_| LargeImageError::SizeOverflow)?;
        let mut pixels = vec![0_u8; len];
        for output_y in 0..tile.height {
            if cancellation.is_cancelled() {
                return Err(LargeImageError::Cancelled);
            }
            let source_y = tile
                .y
                .checked_add(output_y)
                .and_then(|value| value.checked_mul(level.divisor()))
                .map(|value| value.min(self.layout.height() - 1))
                .ok_or(LargeImageError::SizeOverflow)?;
            let source_row = self
                .reader
                .row(source_y)
                .ok_or(LargeImageError::InvalidPixelData)?;
            for output_x in 0..tile.width {
                let source_x = tile
                    .x
                    .checked_add(output_x)
                    .and_then(|value| value.checked_mul(level.divisor()))
                    .map(|value| value.min(self.layout.width() - 1))
                    .ok_or(LargeImageError::SizeOverflow)?;
                let source = usize::try_from(source_x)
                    .ok()
                    .and_then(|value| value.checked_mul(4))
                    .ok_or(LargeImageError::SizeOverflow)?;
                let destination = usize::try_from(output_y)
                    .ok()
                    .and_then(|row| row.checked_mul(usize::try_from(tile.width).ok()?))
                    .and_then(|pixel| pixel.checked_add(usize::try_from(output_x).ok()?))
                    .and_then(|pixel| pixel.checked_mul(4))
                    .ok_or(LargeImageError::SizeOverflow)?;
                pixels[destination..destination + 4]
                    .copy_from_slice(&source_row[source..source + 4]);
            }
        }
        Ok(DecodedImage {
            pixels_bgra8: pixels,
            width: tile.width,
            height: tile.height,
        })
    }
}

pub fn build_large_image_raster(
    path: &Path,
    cache_dir: &Path,
    cancellation: &DecodeCancellation,
) -> Result<LargeImageRaster, LargeImageError> {
    if cancellation.is_cancelled() {
        return Err(LargeImageError::Cancelled);
    }
    let image_reader = image::ImageReader::open(path)?.with_guessed_format()?;
    let format = image_reader.format();
    let (width, height) = image_reader.into_dimensions()?;
    let layout = RasterLayout::new(width, height)?;
    let key = RasterCacheKey::from_source(path)?;
    let finished = cache_dir.join(format!("{}.bgra", key.as_str()));
    if let Ok(reader) = RasterCacheReader::open(&finished, layout) {
        return Ok(LargeImageRaster {
            path: finished,
            layout,
            reader: Arc::new(reader),
        });
    }

    let path = if format == Some(ImageFormat::Png) {
        match write_png_bgra_raster(path, cache_dir, key.as_str(), layout, cancellation)? {
            Some(path) => path,
            None => write_mapped_bgra_raster(path, cache_dir, key.as_str(), layout, cancellation)?,
        }
    } else {
        write_mapped_bgra_raster(path, cache_dir, key.as_str(), layout, cancellation)?
    };
    let reader = RasterCacheReader::open(&path, layout)?;
    Ok(LargeImageRaster {
        path,
        layout,
        reader: Arc::new(reader),
    })
}

fn write_png_bgra_raster(
    path: &Path,
    cache_dir: &Path,
    key: &str,
    layout: RasterLayout,
    cancellation: &DecodeCancellation,
) -> Result<Option<PathBuf>, LargeImageError> {
    let mut decoder = Decoder::new(BufReader::new(File::open(path)?));
    decoder.set_transformations(Transformations::EXPAND | Transformations::STRIP_16);
    let mut reader = decoder.read_info()?;
    if reader.info().interlaced {
        return Ok(None);
    }
    let (color_type, bit_depth) = reader.output_color_type();
    if bit_depth != BitDepth::Eight {
        return Err(LargeImageError::InvalidPixelData);
    }
    let channels = png_channels(color_type);
    let mut writer = RasterCacheWriter::create(cache_dir, key, layout)?;
    for y in 0..layout.height() {
        if cancellation.is_cancelled() {
            return Err(LargeImageError::Cancelled);
        }
        let source = reader
            .next_row()?
            .ok_or(LargeImageError::InvalidPixelData)?;
        let expected = usize::try_from(layout.width())
            .ok()
            .and_then(|width| width.checked_mul(channels))
            .ok_or(LargeImageError::SizeOverflow)?;
        if source.data().len() < expected {
            return Err(LargeImageError::InvalidPixelData);
        }
        let output = writer.row_mut(y).ok_or(LargeImageError::InvalidPixelData)?;
        for x in 0..layout.width() {
            let source_offset = usize::try_from(x)
                .ok()
                .and_then(|value| value.checked_mul(channels))
                .ok_or(LargeImageError::SizeOverflow)?;
            let destination = usize::try_from(x)
                .ok()
                .and_then(|value| value.checked_mul(4))
                .ok_or(LargeImageError::SizeOverflow)?;
            output[destination..destination + 4].copy_from_slice(&png_pixel(
                source.data(),
                source_offset,
                color_type,
            ));
        }
    }
    Ok(Some(writer.finish()?))
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf, time::SystemTime};

    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};

    use super::*;
    use crate::{DecodeCancellation, TileCoordinate};

    fn temp_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("lumia-large-tiles-{nonce}-{name}"))
    }

    #[test]
    fn raster_cache_builds_once_and_reads_edge_and_scaled_tiles() {
        let dir = temp_dir("raster");
        let source = dir.join("source.png");
        let cache = dir.join("cache");
        fs::create_dir_all(&dir).unwrap();
        let image = RgbaImage::from_fn(1000, 700, |x, y| {
            Rgba([(x % 251) as u8, (y % 241) as u8, 77, 255])
        });
        DynamicImage::ImageRgba8(image)
            .save_with_format(&source, ImageFormat::Png)
            .unwrap();
        let cancellation = DecodeCancellation::default();

        let raster = build_large_image_raster(&source, &cache, &cancellation).unwrap();
        assert_eq!(raster.dimensions(), (1000, 700));
        assert!(raster.path().exists());
        let same = build_large_image_raster(&source, &cache, &cancellation).unwrap();
        assert_eq!(same.path(), raster.path());

        let edge = raster
            .decode_tile(TileCoordinate::new(0, 1, 1), 512, &cancellation)
            .unwrap();
        assert_eq!((edge.width, edge.height), (488, 188));
        assert_eq!(edge.pixels_bgra8.len(), 488 * 188 * 4);

        let half = raster
            .decode_tile(TileCoordinate::new(1, 0, 0), 512, &cancellation)
            .unwrap();
        assert_eq!((half.width, half.height), (500, 350));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn gutter_tiles_share_neighbor_pixels_at_seams() {
        let dir = temp_dir("gutter");
        let source = dir.join("source.png");
        let cache = dir.join("cache");
        fs::create_dir_all(&dir).unwrap();
        let image = RgbaImage::from_fn(1025, 3, |x, y| {
            Rgba([(x % 251) as u8, (y % 241) as u8, 77, 255])
        });
        DynamicImage::ImageRgba8(image)
            .save_with_format(&source, ImageFormat::Png)
            .unwrap();
        let cancellation = DecodeCancellation::default();
        let raster = build_large_image_raster(&source, &cache, &cancellation).unwrap();

        let left = raster
            .decode_tile_with_gutter(TileCoordinate::new(0, 0, 0), 512, 1, &cancellation)
            .unwrap();
        let right = raster
            .decode_tile_with_gutter(TileCoordinate::new(0, 1, 0), 512, 1, &cancellation)
            .unwrap();
        assert_eq!((left.width, left.height), (513, 3));
        assert_eq!((right.width, right.height), (514, 3));

        fn pixel(image: &DecodedImage, x: usize, y: usize) -> &[u8] {
            let start = (y * image.width as usize + x) * 4;
            &image.pixels_bgra8[start..start + 4]
        }
        assert_eq!(pixel(&left, 511, 1), pixel(&right, 0, 1));
        assert_eq!(pixel(&left, 512, 1), pixel(&right, 1, 1));

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn cancelled_raster_and_tile_work_stops() {
        let dir = temp_dir("cancel");
        let source = dir.join("source.png");
        fs::create_dir_all(&dir).unwrap();
        DynamicImage::ImageRgba8(RgbaImage::new(2, 2))
            .save_with_format(&source, ImageFormat::Png)
            .unwrap();
        let cancellation = DecodeCancellation::default();
        cancellation.cancel();
        assert!(matches!(
            build_large_image_raster(&source, &dir.join("cache"), &cancellation),
            Err(LargeImageError::Cancelled)
        ));
        fs::remove_dir_all(dir).unwrap();
    }
}
