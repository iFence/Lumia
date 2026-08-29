use std::path::Path;

use crate::{ImageDocument, ImageSource, ViewportState};

#[derive(Debug, Default)]
pub struct ViewerSession {
    document: Option<ImageDocument>,
    viewport: ViewportState,
    rotation_quarter_turns: u8,
    pending_fit_reset: bool,
}

impl ViewerSession {
    /// Point the session at a new document.
    ///
    /// The fit reset is *deferred*. The outgoing document's pixels stay on
    /// screen until the incoming one finishes decoding, so resetting here
    /// would re-scale and re-centre the image the user is still looking at —
    /// a visible jump on every navigation. Consumers apply the reset with
    /// [`take_pending_fit_reset`](Self::take_pending_fit_reset) once the new
    /// image actually replaces the old one.
    pub fn replace_document(&mut self, document: ImageDocument) {
        self.document = Some(document);
        self.pending_fit_reset = true;
        self.rotation_quarter_turns = 0;
    }

    /// Consume a fit reset deferred by `replace_document`, returning whether
    /// one was outstanding. Returns `false` on every later call until the
    /// next document swap, so it is safe to call on every animation frame.
    pub fn take_pending_fit_reset(&mut self) -> bool {
        std::mem::take(&mut self.pending_fit_reset)
    }

    pub fn clear(&mut self) {
        self.document = None;
        self.viewport = ViewportState::default();
        self.rotation_quarter_turns = 0;
        self.pending_fit_reset = false;
    }

    pub fn document(&self) -> Option<&ImageDocument> {
        self.document.as_ref()
    }

    pub fn document_mut(&mut self) -> Option<&mut ImageDocument> {
        self.document.as_mut()
    }

    pub fn has_document(&self) -> bool {
        self.document.is_some()
    }

    pub fn viewport(&self) -> &ViewportState {
        &self.viewport
    }

    pub fn viewport_mut(&mut self) -> &mut ViewportState {
        &mut self.viewport
    }

    pub fn rotation_quarter_turns(&self) -> u8 {
        self.rotation_quarter_turns
    }

    pub fn rotate_by(&mut self, quarter_turns: u8) {
        self.rotation_quarter_turns = (self.rotation_quarter_turns + quarter_turns) % 4;
        self.viewport.reset_fit();
        // This reset already did the work a deferred one would have done.
        self.pending_fit_reset = false;
    }

    pub fn image_path(&self) -> Option<&Path> {
        match self.document.as_ref().map(|document| &document.source) {
            Some(ImageSource::LocalPath(path) | ImageSource::TemporaryPath(path)) => Some(path),
            None => None,
        }
    }

    pub fn display_dimensions(&self) -> Option<(u32, u32)> {
        let metadata = self.document()?.metadata.as_ref()?;
        if self.rotation_quarter_turns % 2 == 1 {
            Some((metadata.height, metadata.width))
        } else {
            Some((metadata.width, metadata.height))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{ColorDescription, ImageMetadata, PixelFormat, TransferFunction, ViewportState};

    use super::*;

    fn document() -> ImageDocument {
        let mut document = ImageDocument::from_path("image.png");
        document.metadata = Some(ImageMetadata {
            width: 640,
            height: 480,
            color: ColorDescription {
                pixel_format: PixelFormat::U8,
                transfer: TransferFunction::Srgb,
                has_alpha: false,
            },
            format_name: Some("png".into()),
            exif: Default::default(),
        });
        document
    }

    #[test]
    fn replacing_document_defers_the_fit_reset_until_the_new_image_lands() {
        let mut session = ViewerSession::default();
        session.replace_document(document());
        assert!(session.take_pending_fit_reset());
        session.rotate_by(1);
        assert_eq!(session.display_dimensions(), Some((480, 640)));
        session.viewport_mut().set_zoom(2.0);
        session.viewport_mut().pan_by(10.0, 20.0);

        // The outgoing image is still on screen, so its transform survives.
        session.replace_document(document());
        assert_eq!(session.viewport().zoom, 2.0);
        assert_eq!(session.viewport().pan_x, 10.0);
        assert_eq!(session.rotation_quarter_turns(), 0);
        assert_eq!(session.display_dimensions(), Some((640, 480)));

        assert!(session.take_pending_fit_reset());
        session.viewport_mut().reset_fit();
        assert_eq!(session.viewport(), &ViewportState::default());
        // The reset is consumed exactly once.
        assert!(!session.take_pending_fit_reset());
    }

    #[test]
    fn rotating_cancels_a_pending_fit_reset() {
        let mut session = ViewerSession::default();
        session.replace_document(document());
        assert!(session.take_pending_fit_reset());

        session.replace_document(document());
        session.rotate_by(1);
        assert!(!session.take_pending_fit_reset());
        assert_eq!(session.rotation_quarter_turns(), 1);
    }

    #[test]
    fn clearing_removes_document_and_resets_transform() {
        let mut session = ViewerSession::default();
        session.replace_document(document());
        session.viewport_mut().set_zoom(2.0);
        session.rotate_by(1);
        assert!(session.has_document());

        session.clear();
        assert!(!session.has_document());
        assert_eq!(session.image_path(), None);
        assert_eq!(session.viewport(), &ViewportState::default());
        assert_eq!(session.rotation_quarter_turns(), 0);
    }
}
