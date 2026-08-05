use std::{fs::File, io::BufReader, path::Path, time::Duration};

use image::{
    codecs::{gif::GifDecoder, png::PngDecoder, webp::WebPDecoder},
    metadata::LoopCount,
    AnimationDecoder, ImageDecoder, ImageError, Limits,
};

use super::{
    checked_bgra8_len, decoded_image_from_rgba, AnimatedImageFormat, DecodeCancellation,
    DecodedAnimationFrame, ImageLoadError,
};

const MIN_FRAME_DELAY: Duration = Duration::from_millis(10);

pub fn probe_animation_format(path: &Path) -> Result<Option<AnimatedImageFormat>, ImageLoadError> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default();
    if extension.eq_ignore_ascii_case("gif") {
        return Ok(Some(AnimatedImageFormat::Gif));
    }
    if extension.eq_ignore_ascii_case("png") || extension.eq_ignore_ascii_case("apng") {
        let reader = open_buffered(path).map_err(|source| animation_error(path, source))?;
        let decoder = PngDecoder::new(reader).map_err(|source| animation_error(path, source))?;
        return decoder
            .is_apng()
            .map(|animated| animated.then_some(AnimatedImageFormat::Png))
            .map_err(|source| animation_error(path, source));
    }
    if extension.eq_ignore_ascii_case("webp") {
        let reader = open_buffered(path).map_err(|source| animation_error(path, source))?;
        let decoder = WebPDecoder::new(reader).map_err(|source| animation_error(path, source))?;
        return Ok(decoder.has_animation().then_some(AnimatedImageFormat::WebP));
    }
    Ok(None)
}

pub fn stream_animation_frames(
    path: &Path,
    format: AnimatedImageFormat,
    max_frame_bytes: u64,
    cancellation: &DecodeCancellation,
    emit: impl FnMut(DecodedAnimationFrame) -> bool,
) -> Result<(), ImageLoadError> {
    match format {
        AnimatedImageFormat::Gif => stream_decoder(
            path,
            max_frame_bytes,
            cancellation,
            || {
                let mut decoder = GifDecoder::new(open_buffered(path)?)?;
                let dimensions = decoder.dimensions();
                let mut limits = decoder_limits(max_frame_bytes);
                limits.max_alloc = Some(max_frame_bytes.saturating_mul(2));
                decoder.set_limits(limits)?;
                Ok((decoder, dimensions))
            },
            emit,
        ),
        AnimatedImageFormat::Png => stream_decoder(
            path,
            max_frame_bytes,
            cancellation,
            || {
                let decoder = PngDecoder::with_limits(
                    open_buffered(path)?,
                    decoder_limits(max_frame_bytes.saturating_mul(3)),
                )?;
                let dimensions = decoder.dimensions();
                Ok((decoder.apng()?, dimensions))
            },
            emit,
        ),
        AnimatedImageFormat::WebP => stream_decoder(
            path,
            max_frame_bytes,
            cancellation,
            || {
                let decoder = WebPDecoder::new(open_buffered(path)?)?;
                let dimensions = decoder.dimensions();
                Ok((decoder, dimensions))
            },
            emit,
        ),
    }
}

fn stream_decoder<D>(
    path: &Path,
    max_frame_bytes: u64,
    cancellation: &DecodeCancellation,
    mut open: impl FnMut() -> Result<(D, (u32, u32)), ImageError>,
    mut emit: impl FnMut(DecodedAnimationFrame) -> bool,
) -> Result<(), ImageLoadError>
where
    D: AnimationDecoder<'static> + 'static,
{
    let (mut decoder, dimensions) = open().map_err(|source| animation_error(path, source))?;
    validate_frame_budget(dimensions, max_frame_bytes)?;
    let repeat = finite_loop_count(decoder.loop_count());
    let mut completed_loops = 0_u32;
    loop {
        if cancellation.is_cancelled() {
            return Err(ImageLoadError::Cancelled);
        }
        for frame in decoder.into_frames() {
            if cancellation.is_cancelled() {
                return Err(ImageLoadError::Cancelled);
            }
            let frame = frame.map_err(|source| animation_error(path, source))?;
            let delay = Duration::from(frame.delay()).max(MIN_FRAME_DELAY);
            let buffer = frame.into_buffer();
            let width = buffer.width();
            let height = buffer.height();
            validate_frame_budget((width, height), max_frame_bytes)?;
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
        let opened = open().map_err(|source| animation_error(path, source))?;
        validate_frame_budget(opened.1, max_frame_bytes)?;
        decoder = opened.0;
    }
}

fn open_buffered(path: &Path) -> Result<BufReader<File>, ImageError> {
    File::open(path)
        .map(BufReader::new)
        .map_err(ImageError::IoError)
}

fn decoder_limits(max_alloc: u64) -> Limits {
    let mut limits = Limits::default();
    limits.max_alloc = Some(max_alloc);
    limits
}

fn validate_frame_budget(
    (width, height): (u32, u32),
    max_frame_bytes: u64,
) -> Result<(), ImageLoadError> {
    let frame_bytes = checked_bgra8_len(width, height)
        .and_then(|bytes| u64::try_from(bytes).ok())
        .unwrap_or(u64::MAX);
    if frame_bytes > max_frame_bytes {
        Err(ImageLoadError::MemoryLimit {
            bytes: frame_bytes,
            limit: max_frame_bytes,
        })
    } else {
        Ok(())
    }
}

fn finite_loop_count(loop_count: LoopCount) -> Option<u32> {
    match loop_count {
        LoopCount::Infinite => None,
        LoopCount::Finite(count) => Some(count.get()),
    }
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
        stream_animation_frames(
            &path,
            AnimatedImageFormat::Gif,
            1024,
            &DecodeCancellation::default(),
            |frame| {
                frames.push(frame);
                frames.len() < 2
            },
        )
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
            stream_animation_frames(
                &path,
                AnimatedImageFormat::Gif,
                15,
                &DecodeCancellation::default(),
                |_| true
            ),
            Err(ImageLoadError::MemoryLimit { .. })
        ));
        let cancellation = DecodeCancellation::default();
        cancellation.cancel();
        assert!(matches!(
            stream_animation_frames(&path, AnimatedImageFormat::Gif, 16, &cancellation, |_| true),
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
        stream_animation_frames(
            &path,
            AnimatedImageFormat::Gif,
            16,
            &DecodeCancellation::default(),
            |_| {
                frame_count += 1;
                true
            },
        )
        .unwrap();

        assert_eq!(frame_count, 1);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn apng_is_detected_and_streamed_with_declared_delays() {
        let path = fixture_path("animated").with_extension("apng");
        let file = File::create(&path).unwrap();
        let mut encoder = png::Encoder::new(file, 1, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_animated(2, 1).unwrap();
        encoder.set_frame_delay(2, 100).unwrap();
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[255, 0, 0, 255]).unwrap();
        writer.set_frame_delay(3, 100).unwrap();
        writer.write_image_data(&[0, 255, 0, 255]).unwrap();
        writer.finish().unwrap();

        assert_eq!(
            probe_animation_format(&path).unwrap(),
            Some(AnimatedImageFormat::Png)
        );
        let mut frames = Vec::new();
        stream_animation_frames(
            &path,
            AnimatedImageFormat::Png,
            16,
            &DecodeCancellation::default(),
            |frame| {
                frames.push(frame);
                true
            },
        )
        .unwrap();

        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].image.pixels_bgra8, [0, 0, 255, 255]);
        assert_eq!(frames[0].delay, Duration::from_millis(20));
        assert_eq!(frames[1].delay, Duration::from_millis(30));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn animated_webp_is_detected_and_streamed() {
        const ANIMATED_WEBP: &[u8] = &[
            0x52, 0x49, 0x46, 0x46, 0x9e, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50, 0x56, 0x50,
            0x38, 0x58, 0x0a, 0x00, 0x00, 0x00, 0x12, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x01,
            0x00, 0x00, 0x41, 0x4e, 0x49, 0x4d, 0x06, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff,
            0x01, 0x00, 0x41, 0x4e, 0x4d, 0x46, 0x36, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0xf4, 0x01, 0x00, 0x02, 0x56, 0x50,
            0x38, 0x4c, 0x1e, 0x00, 0x00, 0x00, 0x2f, 0x01, 0x40, 0x00, 0x00, 0x17, 0x30, 0xff,
            0x02, 0x82, 0x22, 0xff, 0x47, 0x9b, 0xff, 0xf9, 0x0f, 0x34, 0x0b, 0x0a, 0xdb, 0xb6,
            0x41, 0x61, 0x71, 0x10, 0xd1, 0xff, 0xc8, 0x03, 0x41, 0x4e, 0x4d, 0x46, 0x34, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00,
            0xf4, 0x01, 0x00, 0x00, 0x56, 0x50, 0x38, 0x4c, 0x1c, 0x00, 0x00, 0x00, 0x2f, 0x01,
            0x40, 0x00, 0x10, 0x17, 0x20, 0x10, 0x48, 0x61, 0x93, 0x3f, 0xff, 0x02, 0x82, 0x22,
            0xff, 0x47, 0x9b, 0xff, 0x80, 0xbd, 0xc1, 0x18, 0x44, 0xf4, 0x3f, 0x04,
        ];
        let path = fixture_path("animated").with_extension("webp");
        fs::write(&path, ANIMATED_WEBP).unwrap();

        assert_eq!(
            probe_animation_format(&path).unwrap(),
            Some(AnimatedImageFormat::WebP)
        );
        let mut frame_count = 0;
        stream_animation_frames(
            &path,
            AnimatedImageFormat::WebP,
            16,
            &DecodeCancellation::default(),
            |_| {
                frame_count += 1;
                true
            },
        )
        .unwrap();

        assert_eq!(frame_count, 2);
        fs::remove_file(path).unwrap();
    }
}
