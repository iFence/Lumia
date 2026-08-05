use std::{path::PathBuf, time::Duration};

use gpui::Context;
use lumia_core::{AnimatedImageFormat, DecodeCancellation, DecodePolicy, DecodedAnimationFrame};

use crate::{app::LumiaApp, load_state::PreparedImage, util::format_load_error};

const MAX_ANIMATION_FRAME_BYTES: u64 = 48 * 1024 * 1024;

enum AnimationEvent {
    Frame(DecodedAnimationFrame),
    Error(lumia_core::ImageLoadError),
}

impl LumiaApp {
    pub(crate) fn start_current_static_decode(
        &mut self,
        path: PathBuf,
        generation: u64,
        cancellation: DecodeCancellation,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            let decode_path = path.clone();
            let decode_cancellation = cancellation.clone();
            let decoded = cx
                .background_executor()
                .spawn(async move {
                    lumia_core::load_decoded_image_from_path_with_policy(
                        decode_path,
                        DecodePolicy::default(),
                        &decode_cancellation,
                    )
                    .map(PreparedImage::from_decoded)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if !this.loads.finish_decode(generation)
                    || this.image_path() != Some(path.as_path())
                {
                    return;
                }
                match decoded {
                    Ok(image) => {
                        this.loads.set_current_image(generation, image);
                        this.ui.error_message = None;
                        if this.viewer.rotation_quarter_turns() != 0 {
                            this.rebuild_rotated_image(None, cx);
                        }
                    }
                    Err(lumia_core::ImageLoadError::Cancelled) => return,
                    Err(error) => this.ui.error_message = Some(format_load_error(&error)),
                }
                this.release_retired_images(None, cx);
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn start_current_animation_decode(
        &mut self,
        path: PathBuf,
        format: AnimatedImageFormat,
        generation: u64,
        cancellation: DecodeCancellation,
        cx: &mut Context<Self>,
    ) {
        let (sender, receiver) = async_channel::bounded(1);
        let worker_path = path.clone();
        let worker_cancellation = cancellation.clone();
        let _ = std::thread::Builder::new()
            .name("lumia-animation-decode".into())
            .spawn(move || {
                let result = lumia_core::stream_animation_frames(
                    &worker_path,
                    format,
                    MAX_ANIMATION_FRAME_BYTES,
                    &worker_cancellation,
                    |frame| sender.send_blocking(AnimationEvent::Frame(frame)).is_ok(),
                );
                if let Err(error) = result {
                    let _ = sender.send_blocking(AnimationEvent::Error(error));
                }
            });

        cx.spawn(async move |this, cx| {
            let mut first_frame = true;
            let mut fallback_started = false;
            let mut previous_delay = Duration::ZERO;
            loop {
                if !first_frame {
                    cx.background_executor().timer(previous_delay).await;
                }
                while this
                    .read_with(cx, |this, _| {
                        this.loads.is_current(generation)
                            && !cancellation.is_cancelled()
                            && !this.window_active
                    })
                    .unwrap_or(false)
                {
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                }
                let Ok(event) = receiver.recv().await else {
                    break;
                };
                let keep_running = this
                    .update(cx, |this, cx| {
                        if !this.loads.is_current(generation)
                            || this.image_path() != Some(path.as_path())
                            || cancellation.is_cancelled()
                        {
                            return false;
                        }
                        match event {
                            AnimationEvent::Frame(frame) => {
                                previous_delay = frame.delay;
                                this.loads.set_current_image(
                                    generation,
                                    PreparedImage::from_decoded(frame.image),
                                );
                                if first_frame {
                                    this.loads.mark_decode_ready(generation);
                                    this.ui.error_message = None;
                                }
                                if this.viewer.rotation_quarter_turns() != 0 {
                                    this.rebuild_rotated_image(None, cx);
                                }
                                this.release_retired_images(None, cx);
                                cx.notify();
                                true
                            }
                            AnimationEvent::Error(lumia_core::ImageLoadError::Cancelled) => false,
                            AnimationEvent::Error(error) => {
                                if first_frame {
                                    fallback_started = true;
                                    this.start_current_static_decode(
                                        path.clone(),
                                        generation,
                                        cancellation.clone(),
                                        cx,
                                    );
                                } else {
                                    this.loads.finish_decode(generation);
                                    this.ui.error_message = Some(format_load_error(&error));
                                }
                                cx.notify();
                                false
                            }
                        }
                    })
                    .unwrap_or(false);
                if !keep_running {
                    break;
                }
                first_frame = false;
            }
            if !fallback_started {
                let _ = this.update(cx, |this, _| {
                    if this.loads.is_current(generation) {
                        this.loads.finish_decode(generation);
                    }
                });
            }
        })
        .detach();
    }
}
