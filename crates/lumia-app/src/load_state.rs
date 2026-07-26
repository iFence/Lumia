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
}

#[derive(Default)]
pub(crate) struct ImageLoadState {
    current_generation: u64,
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
        self.retire_display_images();
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

    pub(crate) fn set_current_image(&mut self, generation: u64, image: PreparedImage) -> bool {
        if !self.is_current(generation) {
            return false;
        }
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

    pub(crate) fn display_image(&self, quarter_turns: u8) -> Option<&PreparedImage> {
        if quarter_turns % 4 == 0 {
            self.current_image.as_ref()
        } else {
            self.rotated_image.as_ref()
        }
    }

    pub(crate) fn set_rotated_image(&mut self, image: Option<PreparedImage>) {
        if let Some(previous) = self.rotated_image.take() {
            self.retired_images.push(previous);
        }
        self.rotated_image = image;
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

        state.set_rotated_image(Some(prepared(3)));
        state.begin_current_load();
        assert_eq!(state.drain_retired_images().count(), 2);
    }
}
