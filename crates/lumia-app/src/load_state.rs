use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use lumia_core::CachedImage;
#[derive(Default)]
pub(crate) struct ImageLoadState {
    current_generation: u64,
    catalog_generation: u64,
    catalog_paths: HashSet<PathBuf>,
    queued_preloads: HashSet<PathBuf>,
    preload_cache: HashMap<PathBuf, CachedImage>,
    is_decoding: bool,
}

impl ImageLoadState {
    pub(crate) fn begin_current_load(&mut self) -> u64 {
        self.current_generation = self.current_generation.wrapping_add(1);
        self.is_decoding = false;
        self.current_generation
    }

    pub(crate) fn begin_decode(&mut self, generation: u64) {
        if self.current_generation == generation {
            self.is_decoding = true;
        }
    }

    pub(crate) fn finish_decode(&mut self, generation: u64) -> bool {
        if self.current_generation != generation {
            return false;
        }
        self.is_decoding = false;
        true
    }

    pub(crate) fn is_decoding(&self) -> bool {
        self.is_decoding
    }

    pub(crate) fn sync_catalog(&mut self, paths: &[PathBuf]) {
        let next_paths = paths.iter().cloned().collect::<HashSet<_>>();
        if next_paths == self.catalog_paths {
            return;
        }
        self.catalog_generation = self.catalog_generation.wrapping_add(1);
        self.catalog_paths = next_paths;
        self.queued_preloads.clear();
        self.preload_cache
            .retain(|path, _| self.catalog_paths.contains(path));
    }

    pub(crate) fn queue_preload(&mut self, path: PathBuf) -> Option<u64> {
        if !self.catalog_paths.contains(&path)
            || self.preload_cache.contains_key(&path)
            || !self.queued_preloads.insert(path)
        {
            return None;
        }
        Some(self.catalog_generation)
    }

    pub(crate) fn complete_preload(
        &mut self,
        path: PathBuf,
        catalog_generation: u64,
        image: Option<CachedImage>,
    ) -> bool {
        if self.catalog_generation != catalog_generation || !self.catalog_paths.contains(&path) {
            return false;
        }
        self.queued_preloads.remove(&path);
        if let Some(image) = image {
            self.preload_cache.insert(path, image);
        }
        true
    }

    pub(crate) fn take_cached(&mut self, path: &Path) -> Option<CachedImage> {
        self.preload_cache.remove(path)
    }

    pub(crate) fn cached(&self, path: &Path) -> Option<&CachedImage> {
        self.preload_cache.get(path)
    }

    pub(crate) fn retain_cache(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        let retained = paths.into_iter().collect::<HashSet<_>>();
        self.preload_cache.retain(|path, _| retained.contains(path));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cached() -> CachedImage {
        CachedImage {
            cached_data: vec![1],
            width: 1,
            height: 1,
        }
    }

    #[test]
    fn preload_queue_deduplicates_and_rejects_stale_results() {
        let first = PathBuf::from("first.heic");
        let second = PathBuf::from("second.heic");
        let mut state = ImageLoadState::default();
        state.sync_catalog(&[first.clone()]);
        let old_generation = state.queue_preload(first.clone()).unwrap();
        assert!(state.queue_preload(first.clone()).is_none());

        state.sync_catalog(std::slice::from_ref(&second));
        assert!(!state.complete_preload(first, old_generation, Some(cached())));
        let generation = state.queue_preload(second.clone()).unwrap();
        assert!(state.complete_preload(second.clone(), generation, Some(cached())));
        assert!(state.cached(&second).is_some());
    }

    #[test]
    fn current_decode_generation_discards_old_completion() {
        let mut state = ImageLoadState::default();
        let old = state.begin_current_load();
        state.begin_decode(old);
        let current = state.begin_current_load();
        assert!(!state.finish_decode(old));
        state.begin_decode(current);
        assert!(state.is_decoding());
        assert!(state.finish_decode(current));
        assert!(!state.is_decoding());
    }
}
