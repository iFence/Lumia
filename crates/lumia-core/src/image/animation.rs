use std::{fs::File, io::BufReader, path::Path, time::Duration};

use image::{codecs::gif::GifDecoder, AnimationDecoder, ImageDecoder, ImageError, Limits};

use super::{
    checked_bgra8_len, decoded_image_from_rgba, DecodeCancellation, DecodedAnimationFrame,
    ImageLoadError,
};

const MIN_FRAME_DELAY: Duration = Duration::from_millis(10);

pub fn stream_gif_frames(
    path: &Path,
    max_frame_bytes: u64,
    cancellation: &DecodeCancellation,
    mut emit: impl FnMut(DecodedAnimationFrame) -> bool,
) -> Result<(), ImageLoadError> {
    let repeat = gif_repeat(path)?;
    let mut completed_loops = 0_u32;
    loop {
        if cancellation.is_cancelled() {
            return Err(ImageLoadError::Cancelled);
        }
        let file = File::open(path).map_err(|source| ImageLoadError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let mut decoder = GifDecoder::new(BufReader::new(file))
            .map_err(|source| animation_error(path, source))?;
        let (width, height) = decoder.dimensions();
        let frame_bytes = checked_bgra8_len(width, height)
            .and_then(|bytes| u64::try_from(bytes).ok())
            .unwrap_or(u64::MAX);
        if frame_bytes > max_frame_bytes {
            return Err(ImageLoadError::MemoryLimit {
                bytes: frame_bytes,
                limit: max_frame_bytes,
            });
        }
        let mut limits = Limits::default();
        limits.max_alloc = Some(max_frame_bytes.saturating_mul(2));
        decoder
            .set_limits(limits)
            .map_err(|source| animation_error(path, source))?;

        for frame in decoder.into_frames() {
            if cancellation.is_cancelled() {
                return Err(ImageLoadError::Cancelled);
            }
            let frame = frame.map_err(|source| animation_error(path, source))?;
            let delay = Duration::from(frame.delay()).max(MIN_FRAME_DELAY);
            let buffer = frame.into_buffer();
            let decoded = decoded_image_from_rgba(buffer.into_raw(), width, height)?;
            if !emit(DecodedAnimationFrame {
                image: decoded,
                delay,
            }) {
                return Ok(());
            }
        }
        completed_loops = completed_loops.saturating_add(1);
        if repeat.is_some_and(|repeat| completed_loops >= repeat) {
            return Ok(());
        }
    }
}

fn gif_repeat(path: &Path) -> Result<Option<u32>, ImageLoadError> {
    let file = File::open(path).map_err(|source| ImageLoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let decoder = gif::DecodeOptions::new()
        .read_info(BufReader::new(file))
        .map_err(|source| {
            animation_error(
                path,
                ImageError::Decoding(image::error::DecodingError::new(
                    image::ImageFormat::Gif.into(),
                    source,
                )),
            )
        })?;
    Ok(match decoder.repeat() {
        gif::Repeat::Finite(0) | gif::Repeat::Infinite => None,
        gif::Repeat::Finite(repeat) => Some(u32::from(repeat)),
    })
}

fn animation_error(path: &Path, source: ImageError) -> ImageLoadError {
    ImageLoadError::Metadata {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs::{self, File},
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use image::{
        codecs::gif::{GifEncoder, Repeat},
        Delay, Frame, Rgba, RgbaImage,
    };

    use super::*;

    fn fixture_path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("lumia-animation-{nonce}-{name}.gif"))
    }

    #[test]
    fn gif_frames_stream_without_collecting_the_animation() {
        let path = fixture_path("stream");
        let file = File::create(&path).unwrap();
        let mut encoder = GifEncoder::new(file);
        for color in [Rgba([10, 20, 30, 255]), Rgba([40, 50, 60, 255])] {
            encoder
                .encode_frame(Frame::from_parts(
                    RgbaImage::from_pixel(2, 1, color),
                    0,
                    0,
                    Delay::from_numer_denom_ms(1, 1),
                ))
                .unwrap();
        }
        drop(encoder);

        let mut frames = Vec::new();
        stream_gif_frames(&path, 1024, &DecodeCancellation::default(), |frame| {
            frames.push(frame);
            frames.len() < 2
        })
        .unwrap();

        assert_eq!(frames.len(), 2);
        assert_eq!(
            frames[0].image.pixels_bgra8,
            [30, 20, 10, 255, 30, 20, 10, 255]
        );
        assert_eq!(frames[0].delay, MIN_FRAME_DELAY);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn gif_stream_checks_frame_budget_and_cancellation() {
        let path = fixture_path("limits");
        let file = File::create(&path).unwrap();
        let mut encoder = GifEncoder::new(file);
        encoder
            .encode_frame(Frame::new(RgbaImage::new(2, 2)))
            .unwrap();
        drop(encoder);

        assert!(matches!(
            stream_gif_frames(&path, 15, &DecodeCancellation::default(), |_| true),
            Err(ImageLoadError::MemoryLimit { .. })
        ));
        let cancellation = DecodeCancellation::default();
        cancellation.cancel();
        assert!(matches!(
            stream_gif_frames(&path, 16, &cancellation, |_| true),
            Err(ImageLoadError::Cancelled)
        ));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn finite_gif_stops_after_its_declared_loop_count() {
        let path = fixture_path("finite");
        let file = File::create(&path).unwrap();
        let mut encoder = GifEncoder::new(file);
        encoder.set_repeat(Repeat::Finite(1)).unwrap();
        encoder
            .encode_frame(Frame::new(RgbaImage::new(1, 1)))
            .unwrap();
        drop(encoder);

        let mut frame_count = 0;
        stream_gif_frames(&path, 16, &DecodeCancellation::default(), |_| {
            frame_count += 1;
            true
        })
        .unwrap();

        assert_eq!(frame_count, 1);
        fs::remove_file(path).unwrap();
    }
}
