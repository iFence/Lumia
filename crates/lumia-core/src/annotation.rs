#[derive(Debug, Clone, PartialEq)]
pub struct IconAnnotation {
    pub asset_id: String,
    pub x: f32,
    pub y: f32,
    pub size: f32,
    pub color: u32,
    pub opacity: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AnnotationDocument {
    items: Vec<IconAnnotation>,
    redo: Vec<IconAnnotation>,
}

impl AnnotationDocument {
    pub fn items(&self) -> &[IconAnnotation] {
        &self.items
    }

    pub fn place(&mut self, item: IconAnnotation) {
        self.items.push(item);
        self.redo.clear();
    }

    pub fn undo(&mut self) -> bool {
        let Some(item) = self.items.pop() else {
            return false;
        };
        self.redo.push(item);
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(item) = self.redo.pop() else {
            return false;
        };
        self.items.push(item);
        true
    }

    pub fn clear(&mut self) -> bool {
        if self.items.is_empty() {
            return false;
        }
        self.redo.clear();
        self.redo.extend(self.items.drain(..).rev());
        true
    }

    pub fn reset(&mut self) {
        self.items.clear();
        self.redo.clear();
    }

    pub fn rotate_by(&mut self, quarter_turns: u8, width: u32, height: u32) {
        let mut width = width as f32;
        let mut height = height as f32;
        for _ in 0..quarter_turns % 4 {
            for item in self.items.iter_mut().chain(self.redo.iter_mut()) {
                let old_x = item.x;
                item.x = height - item.y;
                item.y = old_x;
            }
            std::mem::swap(&mut width, &mut height);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marker(id: &str) -> IconAnnotation {
        IconAnnotation {
            asset_id: id.to_string(),
            x: 10.0,
            y: 20.0,
            size: 48.0,
            color: 0xff0000,
            opacity: 1.0,
        }
    }

    #[test]
    fn annotation_history_round_trips_committed_items() {
        let mut document = AnnotationDocument::default();
        document.place(marker("pin"));
        document.place(marker("star"));
        assert!(document.undo());
        assert_eq!(document.items(), [marker("pin")]);
        assert!(document.redo());
        assert_eq!(document.items(), [marker("pin"), marker("star")]);
    }

    #[test]
    fn placing_after_undo_discards_redo_branch() {
        let mut document = AnnotationDocument::default();
        document.place(marker("pin"));
        document.undo();
        document.place(marker("check"));
        assert!(!document.redo());
        assert_eq!(document.items(), [marker("check")]);
    }

    #[test]
    fn rotation_keeps_annotations_in_display_coordinates() {
        let mut document = AnnotationDocument::default();
        document.place(marker("pin"));
        document.rotate_by(1, 100, 50);
        assert_eq!((document.items()[0].x, document.items()[0].y), (30.0, 10.0));

        document.rotate_by(3, 50, 100);
        assert_eq!((document.items()[0].x, document.items()[0].y), (10.0, 20.0));
    }
}
