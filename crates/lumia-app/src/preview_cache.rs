use std::{
    collections::{HashMap, VecDeque},
    path::{Path, PathBuf},
};

use gpui::Context;
use lumia_core::{DecodeCancellation, ImageDocument, ImageFileMetadata, ImageMetadata};

use crate::app::LumiaApp;
use crate::large_image::{large_image_cache_dir, should_decode_large_image};
use crate::load_state::PreparedImage;

pub(crate) const PREVIEW_CACHE_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone)]
pub(crate) struct CachedPreview {
    image: PreparedImage,
    metadata: ImageMetadata,
    file: ImageFileMetadata,
    bytes: usize,
}

impl CachedPreview {
    pub(crate) fn new(
        image: PreparedImage,
        metadata: ImageMetadata,
        file: ImageFileMetadata,
    ) -> Self {
        let bytes = image.byte_len();
        Self {
            image,
            metadata,
            file,
            bytes,
        }
    }

    pub(crate) fn image(&self) -> PreparedImage {
        self.image.clone()
    }

    pub(crate) fn metadata(&self) -> &ImageMetadata {
        &self.metadata
    }

    pub(crate) fn file(&self) -> &ImageFileMetadata {
        &self.file
    }
}

pub(crate) struct PreviewCache {
    capacity: usize,
    bytes: usize,
    entries: HashMap<PathBuf, CachedPreview>,
    lru: VecDeque<PathBuf>,
}

impl Default for PreviewCache {
    fn default() -> Self {
        Self::new(PREVIEW_CACHE_BYTES)
    }
}

impl PreviewCache {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            capacity,
            bytes: 0,
            entries: HashMap::new(),
            lru: VecDeque::new(),
        }
    }

    pub(crate) fn lookup(
        &mut self,
        path: &Path,
        file: &ImageFileMetadata,
    ) -> (Option<CachedPreview>, Vec<PreparedImage>) {
        let Some(entry) = self.entries.get(path).cloned() else {
            return (None, Vec::new());
        };
        if entry.file != *file {
            let released = self.remove(path).into_iter().collect();
            return (None, released);
        }
        self.touch(path);
        (Some(entry), Vec::new())
    }

    pub(crate) fn contains(&self, path: &Path) -> bool {
        self.entries.contains_key(path)
    }

    pub(crate) fn insert(
        &mut self,
        path: PathBuf,
        entry: CachedPreview,
        pinned: Option<&Path>,
    ) -> Vec<PreparedImage> {
        let mut released = self.remove(&path).into_iter().collect::<Vec<_>>();
        if entry.bytes > self.capacity {
            released.push(entry.image);
            return released;
        }

        self.bytes += entry.bytes;
        self.lru.push_back(path.clone());
        self.entries.insert(path, entry);
        released.extend(self.evict_to_budget(pinned));
        released
    }

    #[cfg(test)]
    fn bytes(&self) -> usize {
        self.bytes
    }

    fn touch(&mut self, path: &Path) {
        self.lru.retain(|candidate| candidate != path);
        self.lru.push_back(path.to_path_buf());
    }

    fn remove(&mut self, path: &Path) -> Option<PreparedImage> {
        self.lru.retain(|candidate| candidate != path);
        let entry = self.entries.remove(path)?;
        self.bytes = self.bytes.saturating_sub(entry.bytes);
        Some(entry.image)
    }

    fn evict_to_budget(&mut self, pinned: Option<&Path>) -> Vec<PreparedImage> {
        let mut released = Vec::new();
        while self.bytes > self.capacity {
            let Some(candidate) = self
                .lru
                .iter()
                .find(|candidate| pinned.is_none_or(|pinned| candidate.as_path() != pinned))
                .cloned()
            else {
                break;
            };
            if let Some(image) = self.remove(&candidate) {
                released.push(image);
            }
        }
        released
    }
}

pub(crate) struct PreviewPreloadJob {
    pub(crate) generation: u64,
    pub(crate) path: PathBuf,
    pub(crate) cancellation: DecodeCancellation,
}

#[derive(Default)]
pub(crate) struct PreviewPreloadState {
    generation: u64,
    cancellation: Option<DecodeCancellation>,
    queue: VecDeque<PathBuf>,
    active: bool,
}

impl PreviewPreloadState {
    pub(crate) fn restart(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        self.cancel();
        let cancellation = DecodeCancellation::default();
        for path in paths {
            if !self.queue.contains(&path) {
                self.queue.push_back(path);
            }
        }
        self.cancellation = Some(cancellation);
    }

    pub(crate) fn cancel(&mut self) {
        if let Some(cancellation) = self.cancellation.take() {
            cancellation.cancel();
        }
        self.generation = self.generation.wrapping_add(1);
        self.queue.clear();
        self.active = false;
    }

    pub(crate) fn next_job(&mut self) -> Option<PreviewPreloadJob> {
        if self.active {
            return None;
        }
        let path = self.queue.pop_front()?;
        let cancellation = self.cancellation.clone()?;
        self.active = true;
        Some(PreviewPreloadJob {
            generation: self.generation,
            path,
            cancellation,
        })
    }

    pub(crate) fn complete(&mut self, generation: u64) -> bool {
        if !self.is_current(generation) {
            return false;
        }
        self.active = false;
        true
    }

    pub(crate) fn is_current(&self, generation: u64) -> bool {
        self.generation == generation
    }
}

impl LumiaApp {
    pub(crate) fn cancel_preview_preloads(&mut self) {
        self.preview_preloads.cancel();
    }

    pub(crate) fn lookup_cached_preview(
        &mut self,
        path: &Path,
        cx: &mut Context<Self>,
    ) -> Option<CachedPreview> {
        let file = file_metadata(path)?;
        let (entry, released) = self.preview_cache.lookup(path, &file);
        self.release_cached_previews(released, cx);
        entry
    }

    pub(crate) fn store_cached_preview(
        &mut self,
        path: PathBuf,
        image: PreparedImage,
        metadata: ImageMetadata,
        file: ImageFileMetadata,
        cx: &mut Context<Self>,
    ) {
        let pinned = self.image_path().map(Path::to_path_buf);
        let released = self.preview_cache.insert(
            path,
            CachedPreview::new(image, metadata, file),
            pinned.as_deref(),
        );
        self.release_cached_previews(released, cx);
    }

    pub(crate) fn schedule_adjacent_preloads(&mut self, cx: &mut Context<Self>) {
        if self.loads.is_decoding() {
            return;
        }
        let Some(current) = self.image_path().map(Path::to_path_buf) else {
            return;
        };
        if !self.preview_cache.contains(&current) {
            return;
        }

        let direction = if self.navigation_direction < 0 { -1 } else { 1 };
        let mut candidates = Vec::with_capacity(2);
        for step in [direction, -direction] {
            let Some(path) = self
                .navigation
                .step_path(&current, step)
                .map(Path::to_path_buf)
            else {
                continue;
            };
            if path != current
                && is_jpeg_path(&path)
                && !self.preview_cache.contains(&path)
                && !candidates.contains(&path)
            {
                candidates.push(path);
            }
        }

        self.preview_preloads.restart(candidates);
        self.start_next_preview_preload(cx);
    }

    fn start_next_preview_preload(&mut self, cx: &mut Context<Self>) {
        let Some(job) = self.preview_preloads.next_job() else {
            return;
        };
        let generation = job.generation;
        let path = job.path;
        let completion_cancellation = job.cancellation.clone();
        let worker_path = path.clone();
        let worker_cancellation = job.cancellation;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let probe = ImageDocument::probe_from_path(&worker_path).ok()?;
                    let metadata = probe.document.metadata?;
                    if !should_decode_large_image(&worker_path, metadata.width, metadata.height) {
                        return None;
                    }
                    let decoded = lumia_core::decode_large_image_preview(
                        &worker_path,
                        2048,
                        2048,
                        &large_image_cache_dir(),
                        &worker_cancellation,
                    )
                    .ok()?;
                    Some((PreparedImage::from_decoded(decoded), metadata, probe.file))
                })
                .await;

            let _ = this.update(cx, |this, cx| {
                if !this.preview_preloads.complete(generation) {
                    return;
                }
                if !completion_cancellation.is_cancelled() {
                    if let Some((image, metadata, file)) = result {
                        this.store_cached_preview(path, image, metadata, file, cx);
                    }
                    this.start_next_preview_preload(cx);
                }
            });
        })
        .detach();
    }

    fn release_cached_previews(
        &mut self,
        images: impl IntoIterator<Item = PreparedImage>,
        cx: &mut Context<Self>,
    ) {
        for image in images {
            cx.drop_image(image.render_image(), None);
        }
    }
}

fn file_metadata(path: &Path) -> Option<ImageFileMetadata> {
    let metadata = std::fs::metadata(path).ok()?;
    metadata.is_file().then(|| ImageFileMetadata {
        size_bytes: metadata.len(),
        modified: metadata.modified().ok(),
    })
}

fn is_jpeg_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("jpg") || extension.eq_ignore_ascii_case("jpeg")
        })
}
#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use lumia_core::{
        ColorDescription, DecodedImage, ImageMetadata, PixelFormat, TransferFunction,
    };

    use super::*;

    fn prepared(width: u32, height: u32, value: u8) -> PreparedImage {
        PreparedImage::from_decoded(DecodedImage {
            pixels_bgra8: vec![value; width as usize * height as usize * 4],
            width,
            height,
        })
    }

    fn metadata(width: u32, height: u32) -> ImageMetadata {
        ImageMetadata {
            width,
            height,
            color: ColorDescription {
                pixel_format: PixelFormat::U8,
                transfer: TransferFunction::Srgb,
                has_alpha: false,
            },
            format_name: Some("Jpeg".into()),
            exif: Default::default(),
        }
    }

    fn file(seconds: u64) -> ImageFileMetadata {
        ImageFileMetadata {
            size_bytes: seconds,
            modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)),
        }
    }

    fn entry(value: u8, signature: u64) -> CachedPreview {
        CachedPreview::new(prepared(1, 1, value), metadata(1, 1), file(signature))
    }

    #[test]
    fn lru_never_exceeds_the_byte_budget() {
        let mut cache = PreviewCache::new(8);
        cache.insert(PathBuf::from("a.jpg"), entry(1, 1), None);
        cache.insert(PathBuf::from("b.jpg"), entry(2, 2), None);
        assert!(cache.lookup(Path::new("a.jpg"), &file(1)).0.is_some());

        let released = cache.insert(PathBuf::from("c.jpg"), entry(3, 3), None);
        assert_eq!(released.len(), 1);
        assert!(cache.contains(Path::new("a.jpg")));
        assert!(!cache.contains(Path::new("b.jpg")));
        assert!(cache.contains(Path::new("c.jpg")));
        assert_eq!(cache.bytes(), 8);
    }

    #[test]
    fn oversized_and_stale_entries_are_released() {
        let mut cache = PreviewCache::new(8);
        let oversized = CachedPreview::new(prepared(2, 2, 1), metadata(2, 2), file(1));
        assert_eq!(
            cache
                .insert(PathBuf::from("large.jpg"), oversized, None)
                .len(),
            1
        );
        assert_eq!(cache.bytes(), 0);

        cache.insert(PathBuf::from("a.jpg"), entry(1, 1), None);
        let (hit, released) = cache.lookup(Path::new("a.jpg"), &file(2));
        assert!(hit.is_none());
        assert_eq!(released.len(), 1);
        assert_eq!(cache.bytes(), 0);
    }

    #[test]
    fn preloads_are_sequential_and_restart_cancels_old_work() {
        let mut state = PreviewPreloadState::default();
        state.restart([PathBuf::from("next.jpg"), PathBuf::from("previous.jpg")]);
        let old = state.next_job().unwrap();
        assert!(state.next_job().is_none());

        state.restart([PathBuf::from("new-next.jpg")]);
        assert!(old.cancellation.is_cancelled());
        assert!(!state.complete(old.generation));
        let current = state.next_job().unwrap();
        assert_eq!(current.path, PathBuf::from("new-next.jpg"));
        assert!(state.complete(current.generation));
    }
}
