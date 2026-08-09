#[derive(Debug, Clone, PartialEq)]
pub enum Annotation {
    Text {
        text: String,
        x: f32,
        y: f32,
        font_size: f32,
        color: u32,
        opacity: f32,
    },
    Rectangle {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        stroke_width: f32,
        color: u32,
        opacity: f32,
    },
    Step {
        number: u32,
        x: f32,
        y: f32,
        size: f32,
        color: u32,
        opacity: f32,
    },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct AnnotationDocument {
    items: Vec<Annotation>,
    redo: Vec<Annotation>,
}

impl AnnotationDocument {
    pub fn items(&self) -> &[Annotation] {
        &self.items
    }

    pub fn place(&mut self, item: Annotation) {
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

    /// The number the next placed step annotation should carry, derived from
    /// the document itself so undo/redo/clear naturally roll it back.
    pub fn next_step_number(&self) -> u32 {
        self.items
            .iter()
            .filter(|item| matches!(item, Annotation::Step { .. }))
            .count() as u32
            + 1
    }

    pub fn rotate_by(&mut self, quarter_turns: u8, width: u32, height: u32) {
        let mut width = width as f32;
        let mut height = height as f32;
        for _ in 0..quarter_turns % 4 {
            for item in self.items.iter_mut().chain(self.redo.iter_mut()) {
                match item {
                    Annotation::Text { x, y, .. } | Annotation::Step { x, y, .. } => {
                        let old_x = *x;
                        *x = height - *y;
                        *y = old_x;
                    }
                    Annotation::Rectangle {
                        x, y, width: w, height: h, ..
                    } => {
                        let (old_x, old_y, old_w, old_h) = (*x, *y, *w, *h);
                        *x = height - (old_y + old_h);
                        *y = old_x;
                        *w = old_h;
                        *h = old_w;
                    }
                }
            }
            std::mem::swap(&mut width, &mut height);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_marker(text: &str) -> Annotation {
        Annotation::Text {
            text: text.to_string(),
            x: 10.0,
            y: 20.0,
            font_size: 24.0,
            color: 0xff0000,
            opacity: 1.0,
        }
    }

    fn step_marker(number: u32) -> Annotation {
        Annotation::Step {
            number,
            x: 10.0,
            y: 20.0,
            size: 24.0,
            color: 0xff0000,
            opacity: 1.0,
        }
    }

    fn rect_marker() -> Annotation {
        Annotation::Rectangle {
            x: 10.0,
            y: 20.0,
            width: 30.0,
            height: 20.0,
            stroke_width: 4.0,
            color: 0xff0000,
            opacity: 1.0,
        }
    }

    #[test]
    fn annotation_history_round_trips_committed_items() {
        let mut document = AnnotationDocument::default();
        document.place(text_marker("one"));
        document.place(text_marker("two"));
        assert!(document.undo());
        assert_eq!(document.items(), [text_marker("one")]);
        assert!(document.redo());
        assert_eq!(
            document.items(),
            [text_marker("one"), text_marker("two")]
        );
    }

    #[test]
    fn placing_after_undo_discards_redo_branch() {
        let mut document = AnnotationDocument::default();
        document.place(text_marker("one"));
        document.undo();
        document.place(step_marker(1));
        assert!(!document.redo());
        assert_eq!(document.items(), [step_marker(1)]);
    }

    #[test]
    fn rotation_keeps_point_annotations_in_display_coordinates() {
        let mut document = AnnotationDocument::default();
        document.place(text_marker("pin"));
        document.rotate_by(1, 100, 50);
        let Annotation::Text { x, y, .. } = &document.items()[0] else {
            panic!("expected a text annotation");
        };
        assert_eq!((*x, *y), (30.0, 10.0));

        document.rotate_by(3, 50, 100);
        let Annotation::Text { x, y, .. } = &document.items()[0] else {
            panic!("expected a text annotation");
        };
        assert_eq!((*x, *y), (10.0, 20.0));
    }

    #[test]
    fn rotation_swaps_rectangle_extents() {
        let mut document = AnnotationDocument::default();
        document.place(rect_marker());
        document.rotate_by(1, 100, 50);
        let Annotation::Rectangle {
            x, y, width, height, ..
        } = &document.items()[0]
        else {
            panic!("expected a rectangle annotation");
        };
        assert_eq!((*x, *y, *width, *height), (10.0, 10.0, 20.0, 30.0));

        document.rotate_by(3, 50, 100);
        let Annotation::Rectangle {
            x, y, width, height, ..
        } = &document.items()[0]
        else {
            panic!("expected a rectangle annotation");
        };
        assert_eq!((*x, *y, *width, *height), (10.0, 20.0, 30.0, 20.0));
    }

    #[test]
    fn step_numbering_follows_document_state() {
        let mut document = AnnotationDocument::default();
        assert_eq!(document.next_step_number(), 1);

        document.place(step_marker(1));
        assert_eq!(document.next_step_number(), 2);

        assert!(document.undo());
        assert_eq!(document.next_step_number(), 1);

        assert!(document.redo());
        assert_eq!(document.next_step_number(), 2);

        document.place(step_marker(2));
        assert_eq!(document.next_step_number(), 3);

        assert!(document.clear());
        assert_eq!(document.next_step_number(), 1);
    }

    #[test]
    fn step_numbering_ignores_other_annotation_kinds() {
        let mut document = AnnotationDocument::default();
        document.place(text_marker("note"));
        document.place(rect_marker());
        assert_eq!(document.next_step_number(), 1);
        document.place(step_marker(1));
        assert_eq!(document.next_step_number(), 2);
    }
}
