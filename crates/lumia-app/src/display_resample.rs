use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use gpui::{Context, RenderImage, WeakEntity, Window};
use lumia_core::{
    resize_decoded_image, DecodedImage, ImageLoadError, ResampleFilter, ViewportState,
};

use crate::app::LumiaApp;
use crate::load_state::PreparedImage;

/// Quiet period after the last zoom change before a resample job runs, so
/// continuous scroll gestures do not queue repeated full-image resizes.
const RESAMPLE_DEBOUNCE: Duration = Duration::from_millis(90);
/// Upper bound on resampled bitmap size; larger targets keep GPU sampling.
const MAX_RESAMPLE_PIXELS: u64 = 16 * 1024 * 1024;

/// Identifies one display-sized bitmap: source file, filter, and the rounded
/// on-screen pixel size it was produced for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResampleKey {
    pub(crate) path: std::path::PathBuf,
    pub(crate) filter: ResampleFilter,
    pub(crate) width: u32,
    pub(crate) height: u32,
}

/// Which cached preview a background job writes back into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ResampleSlot {
    MainDisplay,
    ComparisonLeft,
    ComparisonRight,
}

pub(crate) struct ResampleRequest {
    key: ResampleKey,
    source: Arc<RenderImage>,
}

/// Caches one filtered, display-sized bitmap per slot and schedules the
/// debounced background jobs that produce it. While a new scale is being
/// processed, the previous bitmap keeps rendering (stretched by the frame),
/// so the selected filter stays visible during zoom gestures.
#[derive(Default)]
pub(crate) struct ResamplePreview {
    key: Option<ResampleKey>,
    /// Key the cached `image` was actually produced for. Distinct from `key`,
    /// which is overwritten as soon as a new job is scheduled.
    ready_key: Option<ResampleKey>,
    pending: bool,
    generation: u64,
    image: Option<PreparedImage>,
    cancel: Option<Arc<AtomicBool>>,
}

impl ResamplePreview {
    /// The bitmap for exactly `key`, if one has finished processing.
    pub(crate) fn ready(&self, key: &ResampleKey) -> Option<PreparedImage> {
        match &self.key {
            Some(current) if *current == *key && !self.pending => self.image.clone(),
            _ => None,
        }
    }

    /// The most recent finished bitmap, but only while it was resampled from
    /// `path`.
    ///
    /// Used as a stopgap so the chosen filter stays visible while a new scale
    /// is processing. It must not cross a document boundary: after navigating
    /// to another file the outgoing image's bitmap would be stretched into the
    /// incoming image's frame, which reads as the old image jumping to a new
    /// scale before the new one appears.
    pub(crate) fn latest_for(&self, path: &Path) -> Option<PreparedImage> {
        match &self.ready_key {
            Some(key) if key.path == path => self.image.clone(),
            _ => None,
        }
    }

    /// Schedule (or drop) the pending request for this preview. Passing
    /// `None` clears any completed or in-flight work.
    pub(crate) fn request(
        &mut self,
        request: Option<ResampleRequest>,
        slot: ResampleSlot,
        handle: WeakEntity<LumiaApp>,
        cx: &mut Context<LumiaApp>,
    ) {
        let Some(request) = request else {
            if self.key.is_some() {
                self.cancel_job();
                *self = Self::default();
            }
            return;
        };
        let already_handled = match &self.key {
            Some(key) => *key == request.key && (self.pending || self.image.is_some()),
            None => false,
        };
        if already_handled {
            return;
        }
        // Supersede any running job and keep the old bitmap on screen until
        // the replacement finishes.
        self.cancel_job();
        self.generation += 1;
        let generation = self.generation;
        self.key = Some(request.key.clone());
        self.pending = true;

        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = Some(cancel.clone());

        cx.spawn(async move |_, cx| {
            cx.background_executor().timer(RESAMPLE_DEBOUNCE).await;
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            let ResampleRequest { key, source } = request;
            let job_key = key.clone();
            let result = cx
                .background_executor()
                .spawn(async move { run_resample_job(source, &job_key) })
                .await;
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            let _ = handle.update(cx, |this, cx| {
                if this.complete_resample(slot, generation, key, result) {
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn cancel_job(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
    }

    fn complete(
        &mut self,
        generation: u64,
        key: ResampleKey,
        result: Result<Option<PreparedImage>, ImageLoadError>,
    ) -> bool {
        if generation != self.generation {
            return false;
        }
        match &self.key {
            Some(current) if *current == key => {}
            _ => return false,
        }
        self.pending = false;
        self.cancel = None;
        // A failed resize leaves the previous bitmap in place.
        match result {
            Ok(prepared) => {
                if prepared.is_some() {
                    self.image = prepared;
                    self.ready_key = Some(key);
                }
                true
            }
            Err(_) => false,
        }
    }
}

fn resolve_slot<'a>(this: &'a mut LumiaApp, slot: ResampleSlot) -> Option<&'a mut ResamplePreview> {
    match slot {
        ResampleSlot::MainDisplay => Some(&mut this.display_resample),
        ResampleSlot::ComparisonLeft => Some(&mut this.comparison.as_mut()?.left_resample),
        ResampleSlot::ComparisonRight => Some(&mut this.comparison.as_mut()?.right_resample),
    }
}

impl LumiaApp {
    pub(crate) fn complete_resample(
        &mut self,
        slot: ResampleSlot,
        generation: u64,
        key: ResampleKey,
        result: Result<Option<PreparedImage>, ImageLoadError>,
    ) -> bool {
        resolve_slot(self, slot)
            .map(|preview| preview.complete(generation, key, result))
            .unwrap_or(false)
    }

    /// Refresh the pending resample requests for the main viewer and, when
    /// comparison mode is active, both comparison panes.
    pub(crate) fn schedule_viewer_resamples(&mut self, window: &Window, cx: &mut Context<Self>) {
        let filter = self.settings.resample_filter;
        let handle = self.self_handle.clone();
        let rotation = self.viewer.rotation_quarter_turns();

        let main_display = self.scaled_image_size(window);
        let base = self.loads.display_image(rotation);
        let main_request = plan_display_resample(base, self.image_path(), filter, main_display);

        // Snapshot before the match: both are immutable reads and
        // `ViewportState` is `Copy`, so this costs nothing.
        let main_viewport = *self.viewer.viewport();
        let (
            left_bounds,
            left_viewport,
            right_bounds,
            right_viewport,
            comparison_path,
            comparison_image,
        ) = match self.comparison.as_ref() {
            Some(state) => (
                state.left_pane_bounds,
                // The left pane is rendered from the main viewer's viewport, so
                // its resample key must be built from that one too. Using
                // `state.viewport` here produced a key the renderer never
                // matches against, leaving the left pane on a stale bitmap.
                main_viewport,
                state.right_pane_bounds,
                state.viewport,
                Some(state.path.clone()),
                state.display_image().cloned(),
            ),
            None => (
                None,
                ViewportState::default(),
                None,
                ViewportState::default(),
                None,
                None,
            ),
        };

        let left_request = match (base.as_ref(), self.image_path(), left_bounds) {
            (Some(source), Some(path), Some(bounds)) => crate::comparison::pane_display_size(
                &left_viewport,
                source.dimensions(),
                Some(bounds.size),
            )
            .and_then(|display| build_request(source, path, filter, display)),
            _ => None,
        };
        let right_request = match (
            comparison_image.as_ref(),
            comparison_path.as_deref(),
            right_bounds,
        ) {
            (Some(source), Some(path), Some(bounds)) => crate::comparison::pane_display_size(
                &right_viewport,
                source.dimensions(),
                Some(bounds.size),
            )
            .and_then(|display| build_request(source, path, filter, display)),
            _ => None,
        };

        self.display_resample
            .request(main_request, ResampleSlot::MainDisplay, handle.clone(), cx);
        if let Some(state) = self.comparison.as_mut() {
            state.left_resample.request(
                left_request,
                ResampleSlot::ComparisonLeft,
                handle.clone(),
                cx,
            );
            state
                .right_resample
                .request(right_request, ResampleSlot::ComparisonRight, handle, cx);
        }
    }
}

fn plan_display_resample(
    source: Option<&PreparedImage>,
    path: Option<&Path>,
    filter: ResampleFilter,
    display_size: Option<(f32, f32)>,
) -> Option<ResampleRequest> {
    let (source, path) = match (source, path) {
        (Some(source), Some(path)) => (source, path),
        _ => return None,
    };
    build_request(source, path, filter, display_size?)
}

fn build_request(
    source: &PreparedImage,
    path: &Path,
    filter: ResampleFilter,
    display_size: (f32, f32),
) -> Option<ResampleRequest> {
    let (source_width, source_height) = source.dimensions();
    let target_width = display_size.0.round().max(1.0) as u32;
    let target_height = display_size.1.round().max(1.0) as u32;
    if target_width == source_width && target_height == source_height {
        return None;
    }
    if u64::from(target_width) * u64::from(target_height) > MAX_RESAMPLE_PIXELS {
        return None;
    }
    Some(ResampleRequest {
        key: ResampleKey {
            path: path.to_path_buf(),
            filter,
            width: target_width,
            height: target_height,
        },
        source: source.render_image(),
    })
}

/// Scale at which an image fits inside a pane of the given size; mirrors the
/// comparison renderer's fit computation.
pub(crate) fn pane_fit_scale(pane_size: gpui::Size<gpui::Pixels>, dimensions: (u32, u32)) -> f32 {
    let width = f32::from(pane_size.width).max(1.0);
    let height = f32::from(pane_size.height).max(1.0);
    (width / dimensions.0 as f32).min(height / dimensions.1 as f32)
}

fn run_resample_job(
    source: Arc<RenderImage>,
    key: &ResampleKey,
) -> Result<Option<PreparedImage>, ImageLoadError> {
    let Some(pixels) = source.as_bytes(0) else {
        return Ok(None);
    };
    let size = source.size(0);
    let Ok(source_width) = u32::try_from(size.width.0) else {
        return Ok(None);
    };
    let Ok(source_height) = u32::try_from(size.height.0) else {
        return Ok(None);
    };
    if source_width == 0 || source_height == 0 {
        return Ok(None);
    }
    let input = DecodedImage {
        pixels_bgra8: pixels.to_vec(),
        width: source_width,
        height: source_height,
    };
    resize_decoded_image(&input, key.filter, key.width, key.height)
        .map(|resized| Some(PreparedImage::from_decoded(resized)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(path: &str, size: u32) -> ResampleKey {
        ResampleKey {
            path: Path::new(path).to_path_buf(),
            filter: ResampleFilter::Lanczos,
            width: size,
            height: size,
        }
    }

    fn finish(preview: &mut ResamplePreview, key: ResampleKey) {
        preview.key = Some(key.clone());
        preview.pending = true;
        // Key size is irrelevant here; a 1x1 bitmap keeps the fixture small.
        let image = PreparedImage::from_decoded(DecodedImage {
            pixels_bgra8: vec![0, 0, 0, 255],
            width: 1,
            height: 1,
        });
        assert!(preview.complete(preview.generation, key, Ok(Some(image))));
    }

    #[test]
    fn latest_for_keeps_the_previous_scale_of_the_same_document() {
        let mut preview = ResamplePreview::default();
        finish(&mut preview, key("a.png", 100));

        // Still the same file, only the scale changed: the stale bitmap is a
        // valid stopgap while the new one is being produced.
        preview.key = Some(key("a.png", 200));
        preview.pending = true;
        assert!(preview.ready(&key("a.png", 200)).is_none());
        assert!(preview.latest_for(Path::new("a.png")).is_some());
    }

    #[test]
    fn latest_for_withholds_the_outgoing_documents_bitmap() {
        // After navigating, the previous file's bitmap must not be stretched
        // into the incoming image's frame.
        let mut preview = ResamplePreview::default();
        finish(&mut preview, key("a.png", 100));
        assert!(preview.latest_for(Path::new("a.png")).is_some());
        assert!(preview.latest_for(Path::new("b.png")).is_none());
    }

    #[test]
    fn latest_for_is_empty_before_any_resample_finishes() {
        let preview = ResamplePreview::default();
        assert!(preview.latest_for(Path::new("a.png")).is_none());
    }
}
