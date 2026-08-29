use std::path::PathBuf;

use gpui::{
    div, img, px, rgb, AnyElement, Bounds, InteractiveElement, IntoElement, ObjectFit,
    ParentElement, Pixels, Styled, StyledImage, Window,
};
use lumia_core::{rotate_bgra8, FitMode, ViewportState};

use crate::app::LumiaApp;
use crate::display_resample::{pane_fit_scale, ResampleKey};
use crate::load_state::PreparedImage;
use crate::palette::Palette;
use crate::util::status_message;

pub(crate) struct ComparisonState {
    pub(crate) path: PathBuf,
    /// Canonical decoded image for the right pane. Always stored unrotated.
    pub(crate) image: Option<PreparedImage>,
    /// Rotated copy of `image` for the current `rotation_quarter_turns`.
    /// Cached here so rotation is a one-off cost on user input rather than a
    /// per-frame pixel permutation.
    rotated_image: Option<PreparedImage>,
    pub(crate) viewport: ViewportState,
    pub(crate) rotation_quarter_turns: u8,
    pub(crate) individual_target: Option<bool>,
    pub(crate) loading: bool,
    pub(crate) error: Option<String>,
    /// Screen-space pane bounds captured each frame, used to anchor
    /// scroll-wheel zoom on the cursor position.
    pub(crate) left_pane_bounds: Option<Bounds<Pixels>>,
    pub(crate) right_pane_bounds: Option<Bounds<Pixels>>,
    /// Screen-space bounds of each pane's image frame, including the pan
    /// offset. Anchoring derives the effective scale from these real pixels.
    pub(crate) left_image_bounds: Option<Bounds<Pixels>>,
    pub(crate) right_image_bounds: Option<Bounds<Pixels>>,
    pub(crate) left_resample: crate::display_resample::ResamplePreview,
    pub(crate) right_resample: crate::display_resample::ResamplePreview,
}

impl ComparisonState {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self {
            path,
            image: None,
            rotated_image: None,
            viewport: ViewportState::default(),
            rotation_quarter_turns: 0,
            individual_target: None,
            loading: true,
            error: None,
            left_pane_bounds: None,
            right_pane_bounds: None,
            left_image_bounds: None,
            right_image_bounds: None,
            left_resample: Default::default(),
            right_resample: Default::default(),
        }
    }

    /// Image the right pane should draw: the rotated copy while a rotation is
    /// active, otherwise the canonical decode. Frame geometry and resample
    /// keys must both derive from this so they agree with the bitmap actually
    /// on screen.
    pub(crate) fn display_image(&self) -> Option<&PreparedImage> {
        if self.rotation_quarter_turns.is_multiple_of(4) {
            self.image.as_ref()
        } else {
            self.rotated_image.as_ref().or(self.image.as_ref())
        }
    }

    /// Rebuild `rotated_image` from `image`. Call after storing a new decode
    /// and after every rotation change.
    pub(crate) fn rebuild_rotated_image(&mut self) {
        self.rotated_image = None;
        if self.rotation_quarter_turns.is_multiple_of(4) {
            return;
        }
        let Some(image) = self.image.as_ref() else {
            return;
        };
        let (width, height) = image.dimensions();
        self.rotated_image = image
            .pixels_bgra8()
            .and_then(|pixels| {
                rotate_bgra8(pixels, width, height, self.rotation_quarter_turns).ok()
            })
            .map(PreparedImage::from_decoded);
    }
}

/// Effective on-screen size of an image in a pane, following the renderer's
/// fit-or-zoom rule. Returns `None` while dimensions are unknown.
pub(crate) fn pane_display_size(
    viewport: &ViewportState,
    dimensions: (u32, u32),
    pane_size: Option<gpui::Size<Pixels>>,
) -> Option<(f32, f32)> {
    let scale = match viewport.fit_mode {
        FitMode::FitToWindow => {
            let pane_size = pane_size?;
            pane_fit_scale(pane_size, dimensions)
        }
        _ => viewport.zoom,
    };
    let width = dimensions.0 as f32 * scale;
    let height = dimensions.1 as f32 * scale;
    (width > 0.0 && height > 0.0).then_some((width, height))
}

fn pane_resample_key(
    source: &PreparedImage,
    path: &std::path::Path,
    filter: lumia_core::ResampleFilter,
    viewport: &ViewportState,
    pane_bounds: Option<Bounds<Pixels>>,
) -> Option<ResampleKey> {
    let (width, height) =
        pane_display_size(viewport, source.dimensions(), pane_bounds.map(|b| b.size))?;
    Some(ResampleKey {
        path: path.to_path_buf(),
        filter,
        width: width.round().max(1.0) as u32,
        height: height.round().max(1.0) as u32,
    })
}

impl LumiaApp {
    /// Effective scale of the viewport comparison input currently targets;
    /// drives the status-bar zoom readout.
    pub(crate) fn comparison_zoom_fraction(&self) -> Option<f32> {
        let state = self.comparison.as_ref()?;
        let rotation = self.viewer.rotation_quarter_turns();
        let (viewport, dimensions, bounds) = match state.individual_target {
            Some(true) => (
                &state.viewport,
                state.display_image().map(PreparedImage::dimensions),
                state.right_pane_bounds,
            ),
            _ => (
                self.viewer.viewport(),
                self.loads
                    .display_image(rotation)
                    .map(PreparedImage::dimensions),
                state.left_pane_bounds,
            ),
        };
        match viewport.fit_mode {
            lumia_core::FitMode::FitToWindow => {
                let dimensions = dimensions?;
                bounds.map(|bounds| pane_fit_scale(bounds.size, dimensions))
            }
            _ => Some(viewport.zoom),
        }
    }

    pub(crate) fn render_comparison(&self, window: &Window, palette: Palette) -> AnyElement {
        let Some(state) = self.comparison.as_ref() else {
            return div().into_any_element();
        };
        let left = self
            .loads
            .display_image(self.viewer.rotation_quarter_turns());
        let right = state.display_image();
        let left_viewport = self.viewer.viewport();
        let right_viewport = &state.viewport;
        let filter = self.settings.resample_filter;

        let left_override = left
            .as_ref()
            .zip(self.image_path())
            .and_then(|(base, path)| {
                pane_resample_key(base, path, filter, left_viewport, state.left_pane_bounds)
            })
            .and_then(|key| state.left_resample.ready(&key));
        let right_override = right
            .and_then(|base| {
                pane_resample_key(
                    base,
                    &state.path,
                    filter,
                    right_viewport,
                    state.right_pane_bounds,
                )
            })
            .and_then(|key| state.right_resample.ready(&key));

        let window_size = window.viewport_size();
        let fallback_pane_size = gpui::size(
            px(f32::from(window_size.width) / 2.0),
            px(f32::from(window_size.height)),
        );
        let left_pane_size = state
            .left_pane_bounds
            .map_or(fallback_pane_size, |b| b.size);
        let right_pane_size = state
            .right_pane_bounds
            .map_or(fallback_pane_size, |b| b.size);

        let pane = |id: &'static str,
                    image: Option<PreparedImage>,
                    source_dims: (u32, u32),
                    viewport: &ViewportState,
                    pane_size: gpui::Size<Pixels>,
                    active: bool,
                    capture: Option<(gpui::WeakEntity<LumiaApp>, bool)>| {
            let mut pane_div = div()
                .flex_1()
                .h_full()
                .relative()
                .overflow_hidden()
                .flex()
                .items_center()
                .justify_center();
            if let Some((handle, is_right)) = capture {
                pane_div = pane_div.on_children_prepainted(move |bounds, _, cx| {
                    if let Some(bounds) = bounds.first() {
                        let _ = handle.update(cx, |this, _| {
                            if let Some(state) = this.comparison.as_mut() {
                                let slot = if is_right {
                                    &mut state.right_image_bounds
                                } else {
                                    &mut state.left_image_bounds
                                };
                                *slot = Some(*bounds);
                            }
                        });
                    }
                });
            }
            let mut pane_div = pane_div.id(id);
            // Frame geometry always derives from the canonical source
            // dimensions so swapping in a resampled bitmap of a different
            // pixel size never shifts or rescales the layout.
            let scale = if viewport.fit_mode == lumia_core::FitMode::FitToWindow {
                pane_fit_scale(pane_size, source_dims)
            } else {
                viewport.zoom
            };
            if let Some(image) = image {
                pane_div = pane_div.child(
                    div()
                        .relative()
                        .left(px(viewport.pan_x))
                        .top(px(viewport.pan_y))
                        .w(px(source_dims.0 as f32 * scale))
                        .h(px(source_dims.1 as f32 * scale))
                        .child(
                            img(image.render_image())
                                .size_full()
                                .object_fit(ObjectFit::Contain),
                        ),
                );
            } else {
                pane_div =
                    pane_div.child(status_message(id, "Loading image...", palette.muted_text));
            }
            if active {
                pane_div = pane_div.border_2().border_color(rgb(palette.accent));
            }
            pane_div.into_any_element()
        };

        let left_source_dims = left.map(PreparedImage::dimensions);
        let right_source_dims = right.map(PreparedImage::dimensions);
        let right = state.display_image().cloned();
        let left_pane = pane(
            "comparison-left",
            left_override
                .or_else(|| {
                    self.image_path()
                        .and_then(|path| state.left_resample.latest_for(path))
                })
                .or_else(|| left.cloned()),
            left_source_dims.unwrap_or((1, 1)),
            left_viewport,
            left_pane_size,
            state.individual_target == Some(false),
            Some((self.self_handle.clone(), false)),
        );
        let right_pane = pane(
            "comparison-right",
            right_override
                .or_else(|| state.right_resample.latest_for(&state.path))
                .or_else(|| right.clone()),
            right_source_dims.unwrap_or((1, 1)),
            right_viewport,
            right_pane_size,
            state.individual_target == Some(true),
            Some((self.self_handle.clone(), true)),
        );

        let capture = self.self_handle.clone();
        let wrap = |element: AnyElement,
                    pick: fn(&mut ComparisonState) -> &mut Option<Bounds<Pixels>>,
                    handle: gpui::WeakEntity<LumiaApp>| {
            div()
                .flex_1()
                .min_w_0()
                .h_full()
                .flex()
                .on_children_prepainted(move |bounds, _, cx| {
                    if let Some(bounds) = bounds.first() {
                        let _ = handle.update(cx, |this, _| {
                            if let Some(state) = this.comparison.as_mut() {
                                *pick(state) = Some(*bounds);
                            }
                        });
                    }
                })
                .child(element)
        };

        div()
            .id("comparison-view")
            .size_full()
            .flex()
            .bg(rgb(palette.viewer_bg))
            .child(wrap(
                left_pane,
                |s| &mut s.left_pane_bounds,
                capture.clone(),
            ))
            .child(div().w(px(1.0)).h_full().bg(rgb(palette.border)))
            .child(wrap(right_pane, |s| &mut s.right_pane_bounds, capture))
            .into_any_element()
    }
}
