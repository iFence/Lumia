use std::{
    collections::{HashSet, VecDeque},
    path::{Path, PathBuf},
};

use lumia_core::{DecodeCancellation, LargeImagePolicy, LargeImageRaster, TileCoordinate};

use crate::tile_cache::TileCache;

const LARGE_IMAGE_TILE_CACHE_BYTES: usize = 256 * 1024 * 1024;

pub(crate) fn should_decode_large_image(path: &Path, width: u32, height: u32) -> bool {
    let excluded = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "heic" | "heif" | "psd" | "psb"
            )
        });
    !excluded && LargeImagePolicy::default().requires_tiling(width, height)
}

pub(crate) fn large_image_cache_dir() -> PathBuf {
    std::env::temp_dir().join("lumia").join("large-images")
}

pub(crate) struct LargeImageSession<T> {
    path: Option<PathBuf>,
    generation: u64,
    cancellation: Option<DecodeCancellation>,
    preview_ready: bool,
    raster: Option<LargeImageRaster>,
    visible_queue: VecDeque<TileCoordinate>,
    prefetch_queue: VecDeque<TileCoordinate>,
    pending: HashSet<TileCoordinate>,
    tiles: TileCache<T>,
    detail_error: Option<String>,
}

impl<T> Default for LargeImageSession<T> {
    fn default() -> Self {
        Self::new(LARGE_IMAGE_TILE_CACHE_BYTES)
    }
}

impl<T> LargeImageSession<T> {
    pub(crate) fn new(tile_cache_bytes: usize) -> Self {
        Self {
            path: None,
            generation: 0,
            cancellation: None,
            preview_ready: false,
            raster: None,
            visible_queue: VecDeque::new(),
            prefetch_queue: VecDeque::new(),
            pending: HashSet::new(),
            tiles: TileCache::new(tile_cache_bytes),
            detail_error: None,
        }
    }

    pub(crate) fn begin(
        &mut self,
        path: PathBuf,
        generation: u64,
        cancellation: DecodeCancellation,
    ) {
        self.reset();
        self.path = Some(path);
        self.generation = generation;
        self.cancellation = Some(cancellation);
    }

    pub(crate) fn reset(&mut self) {
        if let Some(cancellation) = self.cancellation.take() {
            cancellation.cancel();
        }
        self.path = None;
        self.preview_ready = false;
        self.raster = None;
        self.visible_queue.clear();
        self.prefetch_queue.clear();
        self.pending.clear();
        self.tiles.clear();
        self.detail_error = None;
    }

    pub(crate) fn matches(&self, generation: u64, path: &Path) -> bool {
        self.generation == generation && self.path.as_deref() == Some(path)
    }

    pub(crate) fn mark_preview_ready(&mut self, generation: u64) -> bool {
        if self.generation != generation || self.path.is_none() {
            return false;
        }
        self.preview_ready = true;
        true
    }

    pub(crate) const fn is_preview_ready(&self) -> bool {
        self.preview_ready
    }

    pub(crate) fn install_raster(&mut self, generation: u64, raster: LargeImageRaster) -> bool {
        if self.generation != generation || self.path.is_none() {
            return false;
        }
        self.raster = Some(raster);
        self.detail_error = None;
        true
    }

    pub(crate) fn raster(&self) -> Option<LargeImageRaster> {
        self.raster.clone()
    }

    pub(crate) fn queue_tiles(
        &mut self,
        visible: impl IntoIterator<Item = TileCoordinate>,
        prefetch: impl IntoIterator<Item = TileCoordinate>,
    ) {
        for coordinate in visible {
            if !self.has_or_queued(coordinate) {
                self.visible_queue.push_back(coordinate);
            }
        }
        for coordinate in prefetch {
            if !self.has_or_queued(coordinate) {
                self.prefetch_queue.push_back(coordinate);
            }
        }
    }

    fn has_or_queued(&self, coordinate: TileCoordinate) -> bool {
        self.tiles.contains(&coordinate)
            || self.pending.contains(&coordinate)
            || self.visible_queue.contains(&coordinate)
            || self.prefetch_queue.contains(&coordinate)
    }

    pub(crate) fn next_tile(&mut self) -> Option<TileCoordinate> {
        let coordinate = self
            .visible_queue
            .pop_front()
            .or_else(|| self.prefetch_queue.pop_front())?;
        self.pending.insert(coordinate);
        Some(coordinate)
    }

    pub(crate) fn complete_tile(
        &mut self,
        generation: u64,
        coordinate: TileCoordinate,
        tile: Option<T>,
        bytes: usize,
    ) -> bool {
        if self.generation != generation || self.path.is_none() {
            return false;
        }
        self.pending.remove(&coordinate);
        if let Some(tile) = tile {
            self.tiles.insert(coordinate, tile, bytes);
        }
        true
    }

    pub(crate) fn tile(&self, coordinate: &TileCoordinate) -> Option<&T> {
        self.tiles.peek(coordinate)
    }

    pub(crate) fn record_detail_error(&mut self, generation: u64, message: String) {
        if self.generation == generation && self.path.is_some() {
            self.detail_error = Some(message);
        }
    }

    pub(crate) fn detail_error(&self) -> Option<&str> {
        self.detail_error.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use lumia_core::{DecodeCancellation, TileCoordinate};

    use super::*;

    #[test]
    fn large_raster_dispatch_excludes_plugin_and_heif_paths() {
        assert!(should_decode_large_image(Path::new("wide.png"), 9000, 100));
        assert!(should_decode_large_image(
            Path::new("dense.webp"),
            9000,
            9000
        ));
        assert!(!should_decode_large_image(
            Path::new("normal.jpg"),
            1000,
            800
        ));
        assert!(!should_decode_large_image(
            Path::new("wide.heic"),
            9000,
            9000
        ));
        assert!(!should_decode_large_image(
            Path::new("wide.psd"),
            9000,
            9000
        ));
    }

    #[test]
    fn beginning_a_new_session_cancels_and_rejects_the_old_generation() {
        let mut session = LargeImageSession::<u8>::new(16);
        let old_cancellation = DecodeCancellation::default();
        session.begin(PathBuf::from("old.png"), 1, old_cancellation.clone());
        session.mark_preview_ready(1);
        assert!(session.is_preview_ready());

        session.begin(PathBuf::from("new.png"), 2, DecodeCancellation::default());
        assert!(old_cancellation.is_cancelled());
        assert!(!session.mark_preview_ready(1));
        assert!(!session.is_preview_ready());
        assert!(session.matches(2, Path::new("new.png")));
    }

    #[test]
    fn visible_tiles_are_queued_before_prefetch_without_duplicates() {
        let mut session = LargeImageSession::<u8>::new(16);
        session.begin(PathBuf::from("image.png"), 7, DecodeCancellation::default());
        let a = TileCoordinate::new(0, 0, 0);
        let b = TileCoordinate::new(0, 1, 0);
        let c = TileCoordinate::new(0, 2, 0);
        session.queue_tiles([a, b, a], [c, b]);
        assert_eq!(session.next_tile(), Some(a));
        assert_eq!(session.next_tile(), Some(b));
        assert_eq!(session.next_tile(), Some(c));
        assert_eq!(session.next_tile(), None);
    }

    #[test]
    fn failed_tile_keeps_preview_and_successful_tiles_obey_lru_budget() {
        let mut session = LargeImageSession::new(8);
        session.begin(PathBuf::from("image.png"), 3, DecodeCancellation::default());
        session.mark_preview_ready(3);
        let a = TileCoordinate::new(0, 0, 0);
        let b = TileCoordinate::new(0, 1, 0);
        assert!(session.complete_tile(3, a, None, 0));
        assert!(session.is_preview_ready());
        assert!(session.complete_tile(3, a, Some(1_u8), 8));
        assert!(session.complete_tile(3, b, Some(2_u8), 8));
        assert!(session.tile(&a).is_none());
        assert_eq!(session.tile(&b), Some(&2));
    }
}
