use std::sync::Arc;

use gpui::RenderImage;
use image::{Frame, RgbaImage};
use lumia_core::{DecodeCancellation, DecodedImage, ImageFileMetadata};

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

    pub(crate) fn byte_len(&self) -> usize {
        self.pixels_bgra8().map_or(0, <[u8]>::len)
    }
}

#[derive(Default)]
pub(crate) struct ImageLoadState {
    current_generation: u64,
    display_generation: u64,
    pending_source_dimensions: Option<(u32, u32)>,
    display_source_dimensions: Option<(u32, u32)>,
    display_rotation_quarter_turns: u8,
    current_image: Option<PreparedImage>,
    rotated_image: Option<PreparedImage>,
    current_cancellation: Option<DecodeCancellation>,
    is_decoding: bool,
    file_metadata: Option<ImageFileMetadata>,
    retired_images: Vec<PreparedImage>,
}

impl ImageLoadState {
    pub(crate) fn begin_current_load(&mut self) -> u64 {
        if let Some(cancellation) = self.current_cancellation.take() {
            cancellation.cancel();
        }
        self.current_generation = self.current_generation.wrapping_add(1);
        self.pending_source_dimensions = None;
        self.file_metadata = None;
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

    pub(crate) fn mark_decode_ready(&mut self, generation: u64) -> bool {
        if self.current_generation != generation {
            return false;
        }
        self.is_decoding = false;
        true
    }

    pub(crate) fn is_current(&self, generation: u64) -> bool {
        self.current_generation == generation
    }

    pub(crate) fn is_decoding(&self) -> bool {
        self.is_decoding
    }

    pub(crate) fn is_transitioning(&self) -> bool {
        self.current_image.is_some() && self.display_generation != self.current_generation
    }

    pub(crate) fn set_source_dimensions(
        &mut self,
        generation: u64,
        dimensions: Option<(u32, u32)>,
    ) -> bool {
        if !self.is_current(generation) {
            return false;
        }
        self.pending_source_dimensions = dimensions;
        true
    }

    pub(crate) fn set_current_image(&mut self, generation: u64, image: PreparedImage) -> bool {
        if !self.is_current(generation) {
            return false;
        }
        self.display_generation = generation;
        self.display_rotation_quarter_turns = 0;
        self.display_source_dimensions = self
            .pending_source_dimensions
            .or_else(|| Some(image.dimensions()));
        if let Some(previous) = self.current_image.replace(image) {
            self.retired_images.push(previous);
        }
        if let Some(rotated) = self.rotated_image.take() {
            self.retired_images.push(rotated);
        }
        true
    }

    pub(crate) fn current_image(&self) -> Option<&PreparedImage> {
        self.current_image.as_ref()
    }

    pub(crate) fn display_source_dimensions(&self, quarter_turns: u8) -> Option<(u32, u32)> {
        let (width, height) = self.display_source_dimensions?;
        let quarter_turns = if self.is_transitioning() {
            self.display_rotation_quarter_turns
        } else {
            quarter_turns
        };
        if quarter_turns % 2 == 1 {
            Some((height, width))
        } else {
            Some((width, height))
        }
    }

    pub(crate) fn display_image(&self, quarter_turns: u8) -> Option<&PreparedImage> {
        let quarter_turns = if self.is_transitioning() {
            self.display_rotation_quarter_turns
        } else {
            quarter_turns
        };
        if quarter_turns % 4 == 0 {
            self.current_image.as_ref()
        } else {
            self.rotated_image.as_ref()
        }
    }

    pub(crate) fn set_rotated_image(&mut self, image: Option<PreparedImage>, quarter_turns: u8) {
        if let Some(previous) = self.rotated_image.take() {
            self.retired_images.push(previous);
        }
        self.rotated_image = image;
        self.display_rotation_quarter_turns = quarter_turns % 4;
    }

    pub(crate) fn set_file_metadata(&mut self, metadata: ImageFileMetadata) {
        self.file_metadata = Some(metadata);
    }

    pub(crate) fn file_metadata(&self) -> Option<&ImageFileMetadata> {
        self.file_metadata.as_ref()
    }

    pub(crate) fn drain_retired_images(&mut self) -> impl Iterator<Item = PreparedImage> + '_ {
        self.retired_images.drain(..)
    }

    pub(crate) fn clear_display_images(&mut self) {
        self.retire_display_images();
        self.display_source_dimensions = None;
    }

    fn retire_display_images(&mut self) {
        if let Some(current) = self.current_image.take() {
            self.retired_images.push(current);
        }
        if let Some(rotated) = self.rotated_image.take() {
            self.retired_images.push(rotated);
        }
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
    fn replacing_display_images_retires_resources_for_gpui_release() {
        let mut state = ImageLoadState::default();
        let generation = state.begin_current_load();
        assert!(state.set_current_image(generation, prepared(1)));
        assert!(state.set_current_image(generation, prepared(2)));
        assert_eq!(state.drain_retired_images().count(), 1);

        state.set_rotated_image(Some(prepared(3)), 1);
        let next = state.begin_current_load();
        assert_eq!(state.drain_retired_images().count(), 0);
        assert!(state.is_transitioning());
        assert_eq!(state.display_image(0).unwrap().dimensions(), (1, 1));
        assert_eq!(state.display_source_dimensions(0), Some((1, 1)));
        assert!(state.set_current_image(next, prepared(4)));
        assert!(!state.is_transitioning());
        assert_eq!(state.drain_retired_images().count(), 2);
    }

    #[test]
    fn clearing_display_images_after_a_new_load_retires_previous_raster() {
        // Mirrors loading an SVG after a raster image: SVG is rendered from its
        // path, so the previous decoded image must be retired and display state
        // cleared, allowing the renderer to fall through to the SVG path.
        let mut state = ImageLoadState::default();
        let raster_generation = state.begin_current_load();
        assert!(state.set_current_image(raster_generation, prepared(1)));
        assert!(state.set_source_dimensions(raster_generation, Some((1, 1))));
        state.finish_decode(raster_generation);

        let svg_generation = state.begin_current_load();
        assert!(state.is_transitioning());
        state.clear_display_images();
        state.finish_decode(svg_generation);

        assert!(!state.is_transitioning());
        assert!(state.current_image().is_none());
        assert!(state.display_image(0).is_none());
        assert_eq!(state.display_source_dimensions(0), None);
        assert_eq!(state.drain_retired_images().count(), 1);
    }

    #[test]
    fn browsing_many_images_does_not_retain_retired_pixel_buffers() {
        const OBSERVED_LARGE_ALLOCATIONS: usize = 1_192;

        let mut state = ImageLoadState::default();
        let generation = state.begin_current_load();
        assert!(state.set_current_image(generation, prepared(0)));

        for value in 1..=OBSERVED_LARGE_ALLOCATIONS {
            let previous = state.current_image().unwrap().render_image();
            assert!(state.set_current_image(generation, prepared(value as u8)));

            let retired = state.drain_retired_images().collect::<Vec<_>>();
            assert_eq!(retired.len(), 1);
            drop(retired);

            assert_eq!(
                Arc::strong_count(&previous),
                1,
                "retired image {value} remained referenced by image load state"
            );
        }

        assert_eq!(state.drain_retired_images().count(), 0);
        assert!(state.current_image().is_some());
    }
}
