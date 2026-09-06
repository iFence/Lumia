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
fn invalidate_drops_the_bitmap_and_makes_a_late_completion_a_no_op() {
    let mut preview = ResamplePreview::default();
    finish(&mut preview, key("a.png", 100));
    assert!(preview.latest_for(Path::new("a.png")).is_some());

    let generation_before = preview.generation;
    preview.invalidate();

    assert!(preview.latest_for(Path::new("a.png")).is_none());
    assert!(preview.ready(&key("a.png", 100)).is_none());

    // The navigation case: the pane already reports the new path, so a job
    // scheduled against it before the decode landed must not repopulate
    // the cache with the outgoing image's pixels.
    let stale = key("b.png", 100);
    let image = PreparedImage::from_decoded(DecodedImage {
        pixels_bgra8: vec![1, 2, 3, 255],
        width: 1,
        height: 1,
    });
    assert!(!preview.complete(generation_before, stale.clone(), Ok(Some(image))));
    assert!(preview.latest_for(Path::new("b.png")).is_none());
    assert!(preview.image.is_none());
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
