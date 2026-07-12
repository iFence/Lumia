use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gpui::RenderImage;
use image::{Frame, RgbaImage};
use lumia_core::{DecodeCancellation, DecodedImage};

#[derive(Clone)]
pub(crate) struct PreparedImage {
    render_image: Arc<RenderImage>,
    width: u32,
    height: u32,
}

impl PreparedImage {
    pub(crate) fn from_decoded(decoded: DecodedImage) -> Self {
        let buffer = RgbaImage::from_raw(decoded.width, decoded.height, decoded.pixels_bgra8)
            .expect("decoded BGRA image length was validated by lumia-core");
        Self {
            render_image: Arc::new(RenderImage::new(vec![Frame::new(buffer)])),
            width: decoded.width,
            height: decoded.height,
        }
    }

    pub(crate) fn render_image(&self) -> Arc<RenderImage> {
        self.render_image.clone()
    }

    pub(crate) fn pixels_bgra8(&self) -> Option<&[u8]> {
        self.render_image.as_bytes(0)
    }

    pub(crate) fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

pub(crate) struct PreloadJob {
    pub(crate) path: PathBuf,
    pub(crate) generation: u64,
    pub(crate) cancellation: DecodeCancellation,
}

#[derive(Default)]
pub(crate) struct ImageLoadState {
    current_generation: u64,
    catalog_paths: HashSet<PathBuf>,
    current_image: Option<PreparedImage>,
    rotated_image: Option<PreparedImage>,
    preload_cache: HashMap<PathBuf, PreparedImage>,
    preload_queue: VecDeque<PathBuf>,
    active_preload: Option<(PathBuf, DecodeCancellation)>,
    preload_generation: u64,
    current_cancellation: Option<DecodeCancellation>,
    is_decoding: bool,
}

impl ImageLoadState {
    pub(crate) fn begin_current_load(&mut self) -> u64 {
        if let Some(cancellation) = self.current_cancellation.take() {
            cancellation.cancel();
        }
        self.cancel_preloads();
        self.current_generation = self.current_generation.wrapping_add(1);
        self.current_image = None;
        self.rotated_image = None;
        self.is_decoding = false;
        self.current_generation
    }

    pub(crate) fn begin_decode(&mut self, generation: u64) -> Option<DecodeCancellation> {
        if self.current_generation != generation {
            return None;
        }
        let cancellation = DecodeCancellation::default();
        self.current_cancellation = Some(cancellation.clone());
        self.is_decoding = true;
        Some(cancellation)
    }

    pub(crate) fn finish_decode(&mut self, generation: u64) -> bool {
        if self.current_generation != generation {
            return false;
        }
        self.current_cancellation = None;
        self.is_decoding = false;
        true
    }

    pub(crate) fn is_current(&self, generation: u64) -> bool {
        self.current_generation == generation
    }

    pub(crate) fn is_decoding(&self) -> bool {
        self.is_decoding
    }

    pub(crate) fn set_current_image(&mut self, generation: u64, image: PreparedImage) -> bool {
        if !self.is_current(generation) {
            return false;
        }
        self.current_image = Some(image);
        self.rotated_image = None;
        true
    }

    pub(crate) fn current_image(&self) -> Option<&PreparedImage> {
        self.current_image.as_ref()
    }

    pub(crate) fn display_image(&self, quarter_turns: u8) -> Option<&PreparedImage> {
        if quarter_turns % 4 == 0 {
            self.current_image.as_ref()
        } else {
            self.rotated_image.as_ref()
        }
    }

    pub(crate) fn set_rotated_image(&mut self, image: Option<PreparedImage>) {
        self.rotated_image = image;
    }

    pub(crate) fn sync_catalog(&mut self, paths: &[PathBuf]) {
        let next_paths = paths.iter().cloned().collect::<HashSet<_>>();
        if next_paths == self.catalog_paths {
            return;
        }
        self.cancel_preloads();
        self.catalog_paths = next_paths;
        self.preload_cache
            .retain(|path, _| self.catalog_paths.contains(path));
    }

    pub(crate) fn take_cached(&mut self, path: &Path) -> Option<PreparedImage> {
        self.preload_cache.remove(path)
    }

    pub(crate) fn retain_cache(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        let retained = paths.into_iter().collect::<HashSet<_>>();
        self.preload_cache.retain(|path, _| retained.contains(path));
    }

    pub(crate) fn prepare_preloads(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        self.cancel_preloads();
        for path in paths {
            if self.catalog_paths.contains(&path) && !self.preload_cache.contains_key(&path) {
                self.preload_queue.push_back(path);
            }
        }
    }

    pub(crate) fn begin_next_preload(&mut self) -> Option<PreloadJob> {
        if self.active_preload.is_some() {
            return None;
        }
        while let Some(path) = self.preload_queue.pop_front() {
            if !self.catalog_paths.contains(&path) || self.preload_cache.contains_key(&path) {
                continue;
            }
            let cancellation = DecodeCancellation::default();
            self.active_preload = Some((path.clone(), cancellation.clone()));
            return Some(PreloadJob {
                path,
                generation: self.preload_generation,
                cancellation,
            });
        }
        None
    }

    pub(crate) fn complete_preload(
        &mut self,
        path: PathBuf,
        generation: u64,
        image: Option<PreparedImage>,
    ) -> bool {
        let is_active = self
            .active_preload
            .as_ref()
            .is_some_and(|(active_path, _)| active_path == &path);
        if generation != self.preload_generation
            || !is_active
            || !self.catalog_paths.contains(&path)
        {
            return false;
        }
        self.active_preload = None;
        if let Some(image) = image {
            self.preload_cache.insert(path, image);
        }
        true
    }

    fn cancel_preloads(&mut self) {
        if let Some((_, cancellation)) = self.active_preload.take() {
            cancellation.cancel();
        }
        self.preload_queue.clear();
        self.preload_generation = self.preload_generation.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prepared(value: u8) -> PreparedImage {
        PreparedImage::from_decoded(DecodedImage {
            pixels_bgra8: vec![value, 0, 0, 255],
            width: 1,
            height: 1,
        })
    }

    #[test]
    fn current_decode_generation_discards_old_completion_and_cancels_work() {
        let mut state = ImageLoadState::default();
        let old = state.begin_current_load();
        let cancellation = state.begin_decode(old).unwrap();
        let current = state.begin_current_load();
        assert!(cancellation.is_cancelled());
        assert!(!state.finish_decode(old));
        assert!(state.set_current_image(current, prepared(1)));
        assert!(state.current_image().is_some());
    }

    #[test]
    fn preloads_run_serially_and_reject_stale_results() {
        let first = PathBuf::from("first.heic");
        let second = PathBuf::from("second.heic");
        let mut state = ImageLoadState::default();
        state.sync_catalog(&[first.clone(), second.clone()]);
        state.prepare_preloads([first.clone(), second.clone()]);

        let first_job = state.begin_next_preload().unwrap();
        assert_eq!(first_job.path, first);
        assert!(state.begin_next_preload().is_none());
        assert!(state.complete_preload(
            first_job.path.clone(),
            first_job.generation,
            Some(prepared(1)),
        ));

        let second_job = state.begin_next_preload().unwrap();
        state.begin_current_load();
        assert!(second_job.cancellation.is_cancelled());
        assert!(!state.complete_preload(second_job.path, second_job.generation, Some(prepared(2)),));
    }
}
