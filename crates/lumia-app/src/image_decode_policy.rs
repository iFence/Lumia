use std::path::Path;

use lumia_core::AnimatedImageFormat;

use crate::large_image::should_decode_large_image;

pub(crate) const MAX_ANIMATION_FRAME_BYTES: u64 = 48 * 1024 * 1024;

pub(crate) fn should_use_large_image_decode(
    path: &Path,
    width: u32,
    height: u32,
    animation: Option<AnimatedImageFormat>,
) -> bool {
    let decoded_bytes = u64::from(width)
        .saturating_mul(u64::from(height))
        .saturating_mul(4);
    if animation.is_some() {
        decoded_bytes > MAX_ANIMATION_FRAME_BYTES
    } else {
        should_decode_large_image(path, width, height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_policy_covers_common_photos_without_suppressing_animation() {
        assert!(should_use_large_image_decode(
            Path::new("photo.jpg"),
            4032,
            3024,
            None
        ));
        assert!(!should_use_large_image_decode(
            Path::new("clip.gif"),
            4032,
            3024,
            Some(AnimatedImageFormat::Gif)
        ));
        assert!(should_use_large_image_decode(
            Path::new("huge.gif"),
            5000,
            4000,
            Some(AnimatedImageFormat::Gif)
        ));
    }
}
