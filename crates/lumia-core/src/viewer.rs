use std::path::Path;

use crate::{ImageDocument, ImageSource, ViewportState};

#[derive(Debug, Default)]
pub struct ViewerSession {
    document: Option<ImageDocument>,
    viewport: ViewportState,
    rotation_quarter_turns: u8,
}

impl ViewerSession {
    pub fn replace_document(&mut self, document: ImageDocument) {
        self.document = Some(document);
        self.viewport.reset_fit();
        self.rotation_quarter_turns = 0;
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
            format_name: Some("Png".into()),
        });
        document
    }

    #[test]
    fn replacing_document_resets_transform_and_rotation() {
        let mut session = ViewerSession::default();
        session.replace_document(document());
        session.viewport_mut().set_zoom(2.0);
        session.viewport_mut().pan_by(10.0, 20.0);
        session.rotate_by(1);
        assert_eq!(session.display_dimensions(), Some((480, 640)));

        session.replace_document(document());
        assert_eq!(session.viewport(), &ViewportState::default());
        assert_eq!(session.rotation_quarter_turns(), 0);
        assert_eq!(session.display_dimensions(), Some((640, 480)));
    }
}
