use std::{
    collections::{HashSet, VecDeque},
    path::{Path, PathBuf},
};

use lumia_core::{large_image_worker_count, PixelBudget};
use lumia_core::{DecodeCancellation, LargeImagePolicy, LargeImageRaster, TileCoordinate};

use gpui::{Context, Window};

use crate::app::LumiaApp;
use crate::large_image_render::LargeImageViewGeometry;
use crate::load_state::PreparedImage;
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
    active_tiles: usize,
    max_workers: usize,
    pixel_budget: PixelBudget,
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
            active_tiles: 0,
            max_workers: large_image_worker_count(
                std::thread::available_parallelism()
                    .map(usize::from)
                    .unwrap_or(1),
            ),
            pixel_budget: PixelBudget::new(256 * 1024 * 1024),
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
        self.active_tiles = 0;
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
        self.visible_queue.clear();
        self.prefetch_queue.clear();
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
        if self.active_tiles >= self.max_workers {
            return None;
        }
        let coordinate = self
            .visible_queue
            .pop_front()
            .or_else(|| self.prefetch_queue.pop_front())?;
        self.pending.insert(coordinate);
        self.active_tiles += 1;
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
        if self.pending.remove(&coordinate) {
            self.active_tiles = self.active_tiles.saturating_sub(1);
        }
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

    pub(crate) fn is_active(&self, path: &Path) -> bool {
        self.path.as_deref() == Some(path)
    }

    pub(crate) const fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn cancellation(&self) -> Option<DecodeCancellation> {
        self.cancellation.clone()
    }

    pub(crate) fn pixel_budget(&self) -> PixelBudget {
        self.pixel_budget.clone()
    }

    pub(crate) fn clear_tile_requests(&mut self) {
        self.visible_queue.clear();
        self.prefetch_queue.clear();
    }
}

impl LumiaApp {
    pub(crate) fn refresh_large_image_tiles(&mut self, window: &Window, cx: &mut Context<Self>) {
        let Some(path) = self.image_path() else {
            return;
        };
        if !self.large_image.is_active(path) || self.viewer.rotation_quarter_turns() != 0 {
            self.large_image.clear_tile_requests();
            return;
        }
        let Some((width, height)) = self.viewer.display_dimensions() else {
            return;
        };
        let Some(scale) = self.image_display_scale(window) else {
            return;
        };
        if let Some(preview) = self.loads.current_image() {
            let (preview_width, preview_height) = preview.dimensions();
            let preview_scale =
                (preview_width as f32 / width as f32).min(preview_height as f32 / height as f32);
            if scale <= preview_scale {
                self.large_image.clear_tile_requests();
                return;
            }
        }
        let viewport = window.viewport_size();
        let Some(geometry) = LargeImageViewGeometry::calculate(
            width,
            height,
            f32::from(viewport.width),
            f32::from(viewport.height),
            scale,
            self.viewer.viewport().pan_x,
            self.viewer.viewport().pan_y,
            self.viewer.rotation_quarter_turns(),
        ) else {
            return;
        };
        self.large_image
            .queue_tiles(geometry.visible_tiles, geometry.prefetch_tiles);
        self.start_large_image_tile_jobs(cx);
    }

    pub(crate) fn start_large_image_tile_jobs(&mut self, cx: &mut Context<Self>) {
        let Some(raster) = self.large_image.raster() else {
            return;
        };
        while let Some(coordinate) = self.large_image.next_tile() {
            let generation = self.large_image.generation();
            let Some(cancellation) = self.large_image.cancellation() else {
                return;
            };
            let Some(permit) = self.large_image.pixel_budget().try_acquire(512 * 512 * 4) else {
                return;
            };
            let raster = raster.clone();
            cx.spawn(async move |this, cx| {
                let decoded = cx
                    .background_executor()
                    .spawn(async move {
                        let _permit = permit;
                        raster
                            .decode_tile(coordinate, 512, &cancellation)
                            .map(|decoded| {
                                let bytes = decoded.pixels_bgra8.len();
                                (PreparedImage::from_decoded(decoded), bytes)
                            })
                    })
                    .await;
                let _ = this.update(cx, |this, cx| {
                    let (tile, bytes) = match decoded {
                        Ok((tile, bytes)) => (Some(tile), bytes),
                        Err(error) => {
                            this.large_image
                                .record_detail_error(generation, error.to_string());
                            (None, 0)
                        }
                    };
                    if this
                        .large_image
                        .complete_tile(generation, coordinate, tile, bytes)
                    {
                        this.start_large_image_tile_jobs(cx);
                        cx.notify();
                    }
                });
            })
            .detach();
        }
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
