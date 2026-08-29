//! Zoom, pan, and rotation transforms for comparison mode, split from
//! `comparison.rs`, which owns the state and pane rendering.

use std::path::PathBuf;

use gpui::{Context, Pixels, Point, Window};
use lumia_core::{FitMode, ViewportState};

use crate::app::LumiaApp;
use crate::comparison::ComparisonState;
use crate::display_resample::pane_fit_scale;
use crate::load_state::PreparedImage;

/// Convert FitToWindow into an equivalent ActualSize zoom so the following
/// ZOOM_STEP multiplies the scale actually on screen rather than jumping to
/// a raw 1.2x field value.
fn prepare_for_step(viewport: &mut ViewportState, fit_scale: Option<f32>) {
    if viewport.fit_mode == FitMode::FitToWindow {
        if let Some(fit_scale) = fit_scale {
            viewport.set_zoom(fit_scale);
        }
    }
}

fn usable_scale(scale: f32) -> bool {
    scale > 1e-6 && scale.is_finite()
}

/// A cursor offset expressed as a fraction of `extent`, clamped to the frame
/// so an anchor that falls outside it slides to the edge instead of flinging
/// the pane off-screen.
fn relative_position(offset: f32, extent: f32) -> f32 {
    if extent > 1e-6 {
        (offset / extent).clamp(0.0, 1.0)
    } else {
        0.5
    }
}

/// Pan that holds the image point at `fraction` of the frame steady on screen
/// while the display scale moves from `old_scale` to `new_scale`.
///
/// For the pane under the cursor this is exactly "the point under the pointer
/// stays under the pointer". The same fraction applied to a pane that is *not*
/// hovered keeps its proportionally-equivalent point steady, which is what
/// makes the two panes zoom as a pair.
fn anchored_pan(
    pre: ViewportState,
    dimensions: (u32, u32),
    old_scale: f32,
    new_scale: f32,
    fraction: (f32, f32),
) -> (f32, f32) {
    let old_width = dimensions.0 as f32 * old_scale;
    let old_height = dimensions.1 as f32 * old_scale;
    let new_width = dimensions.0 as f32 * new_scale;
    let new_height = dimensions.1 as f32 * new_scale;
    (
        pre.pan_x + (new_width - old_width) * (0.5 - fraction.0),
        pre.pan_y + (new_height - old_height) * (0.5 - fraction.1),
    )
}

impl LumiaApp {
    pub(crate) fn comparison_active(&self) -> bool {
        self.comparison.is_some()
    }

    pub(crate) fn start_comparison(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.is_viewer_blocked()
            || self.image_path().is_none()
            || path == self.image_path().unwrap()
        {
            return;
        }
        self.comparison = Some(ComparisonState::new(path.clone()));
        let handle = self.self_handle.clone();
        cx.spawn(async move |_, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { load_decoded_image_from_path_with_policy(&path) })
                .await;
            let _ = handle.update(cx, |this, cx| {
                let Some(state) = this.comparison.as_mut() else {
                    return;
                };
                state.loading = false;
                match result {
                    Ok(decoded) => {
                        state.image = Some(PreparedImage::from_decoded(decoded));
                        // A rotation requested while this decode was still in
                        // flight applies as soon as the image arrives.
                        state.rebuild_rotated_image();
                    }
                    Err(error) => state.error = Some(error.to_string()),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn toggle_comparison_target(&mut self, cx: &mut Context<Self>) {
        if let Some(state) = self.comparison.as_mut() {
            state.individual_target = match state.individual_target {
                None => Some(true),
                Some(true) => Some(false),
                Some(false) => None,
            };
            cx.notify();
        }
    }

    pub(crate) fn close_comparison(&mut self, cx: &mut Context<Self>) {
        self.comparison = None;
        cx.notify();
    }

    /// One zoom step for every viewport affected by the current target mode,
    /// converting out of FitToWindow first so steps stay proportional.
    fn compare_zoom_step(&mut self, inward: bool, cx: &mut Context<Self>) {
        let left_dims = self
            .loads
            .display_image(self.viewer.rotation_quarter_turns())
            .map(PreparedImage::dimensions);
        let (individual, left_pane, right_pane, right_dims) = match self.comparison.as_ref() {
            Some(state) => (
                state.individual_target,
                state.left_pane_bounds,
                state.right_pane_bounds,
                state.display_image().map(PreparedImage::dimensions),
            ),
            None => return,
        };
        let left_fit = left_pane
            .zip(left_dims)
            .map(|(bounds, dims)| pane_fit_scale(bounds.size, dims));
        let right_fit = right_pane
            .zip(right_dims)
            .map(|(bounds, dims)| pane_fit_scale(bounds.size, dims));

        let Some(state) = self.comparison.as_mut() else {
            return;
        };
        match individual {
            None => {
                prepare_for_step(self.viewer.viewport_mut(), left_fit);
                prepare_for_step(&mut state.viewport, right_fit);
            }
            Some(true) => prepare_for_step(&mut state.viewport, right_fit),
            Some(false) => prepare_for_step(self.viewer.viewport_mut(), left_fit),
        }
        match individual {
            None => {
                let left = self.viewer.viewport_mut();
                if inward {
                    left.zoom_in();
                } else {
                    left.zoom_out();
                }
                if inward {
                    state.viewport.zoom_in();
                } else {
                    state.viewport.zoom_out();
                }
            }
            Some(true) => {
                if inward {
                    state.viewport.zoom_in();
                } else {
                    state.viewport.zoom_out();
                }
            }
            Some(false) => {
                let left = self.viewer.viewport_mut();
                if inward {
                    left.zoom_in();
                } else {
                    left.zoom_out();
                }
            }
        }
        cx.notify();
    }

    pub(crate) fn compare_zoom_in(&mut self, _window: &Window, cx: &mut Context<Self>) {
        self.compare_zoom_step(true, cx);
    }

    pub(crate) fn compare_zoom_out(&mut self, _window: &Window, cx: &mut Context<Self>) {
        self.compare_zoom_step(false, cx);
    }

    /// Toggle fit/actual-size on every viewport comparison input targets.
    pub(crate) fn compare_toggle_fit(&mut self, cx: &mut Context<Self>) {
        let individual = match self.comparison.as_ref() {
            Some(state) => state.individual_target,
            None => return,
        };
        fn toggle(viewport: &mut ViewportState) {
            if viewport.fit_mode == FitMode::FitToWindow {
                viewport.reset_actual_size();
            } else {
                viewport.reset_fit();
            }
        }
        let Some(state) = self.comparison.as_mut() else {
            return;
        };
        match individual {
            None => {
                toggle(self.viewer.viewport_mut());
                toggle(&mut state.viewport);
            }
            Some(true) => toggle(&mut state.viewport),
            Some(false) => toggle(self.viewer.viewport_mut()),
        }
        cx.notify();
    }

    /// Scroll-wheel zoom anchored on the cursor. The hovered pane supplies
    /// the anchor geometry; the resulting pan delta is applied to every
    /// viewport affected by the zoom so paired images stay aligned.
    pub(crate) fn compare_scroll_zoom(
        &mut self,
        inward: bool,
        cursor: Point<Pixels>,
        _window: &Window,
        cx: &mut Context<Self>,
    ) {
        let left_dims = self
            .loads
            .display_image(self.viewer.rotation_quarter_turns())
            .map(PreparedImage::dimensions);
        // The left pane renders from the main viewer's viewport; snapshot it
        // before the match so the partner-pan delta uses the right baseline.
        let main_viewport = *self.viewer.viewport();
        let (
            individual,
            left_pane,
            left_img,
            left_pre,
            right_pane,
            right_img,
            right_dims,
            right_pre,
        ) = match self.comparison.as_ref() {
            Some(state) => (
                state.individual_target,
                state.left_pane_bounds,
                state.left_image_bounds,
                // Pre-zoom pan of the *left* pane, which renders from the main
                // viewer's viewport. Using `state.viewport` here fed the right
                // pane's own pan in as the baseline for the partner shift,
                // desyncing the two panes on every wheel zoom.
                main_viewport,
                state.right_pane_bounds,
                state.right_image_bounds,
                state.display_image().map(PreparedImage::dimensions),
                state.viewport,
            ),
            None => return,
        };
        let hovered_left = left_pane.is_some_and(|bounds| bounds.contains(&cursor));
        let hovered_right = right_pane.is_some_and(|bounds| bounds.contains(&cursor));

        let target_right = match individual {
            Some(individual) => individual,
            None if hovered_left || hovered_right => hovered_right,
            // Cursor over the divider or outside the panes: plain zoom.
            None => {
                self.compare_zoom_step(inward, cx);
                return;
            }
        };

        let (image_bounds, dimensions) = if target_right {
            (right_img, right_dims)
        } else {
            (left_img, left_dims)
        };
        let (Some(image_bounds), Some(dimensions)) = (image_bounds, dimensions) else {
            self.compare_zoom_step(inward, cx);
            return;
        };
        // The anchor is read from the pane the cursor is actually over, not
        // from the pane being zoomed. When they are the same pane this is
        // ordinary cursor-anchored zoom; when individual targeting sends the
        // zoom to the opposite pane it anchors that pane at the
        // proportionally-equivalent point of its own image. Feeding the
        // absolute cursor coordinate into the target pane instead flung it
        // far off-frame whenever the cursor sat outside the target.
        let anchor_bounds = if hovered_right {
            right_img
        } else if hovered_left {
            left_img
        } else {
            // Individual targeting with the cursor off both panes: nothing to
            // read a relative position from, so zoom about the frame centre.
            None
        };
        let fraction = anchor_bounds.map_or((0.5, 0.5), |bounds| {
            (
                relative_position(
                    f32::from(cursor.x) - f32::from(bounds.origin.x),
                    f32::from(bounds.size.width),
                ),
                relative_position(
                    f32::from(cursor.y) - f32::from(bounds.origin.y),
                    f32::from(bounds.size.height),
                ),
            )
        });

        let left_fit = left_pane
            .zip(left_dims)
            .map(|(bounds, dims)| pane_fit_scale(bounds.size, dims));
        let right_fit = right_pane
            .zip(right_dims)
            .map(|(bounds, dims)| pane_fit_scale(bounds.size, dims));
        {
            let Some(state) = self.comparison.as_mut() else {
                return;
            };
            match individual {
                None => {
                    prepare_for_step(self.viewer.viewport_mut(), left_fit);
                    prepare_for_step(&mut state.viewport, right_fit);
                }
                Some(true) => prepare_for_step(&mut state.viewport, right_fit),
                Some(false) => prepare_for_step(self.viewer.viewport_mut(), left_fit),
            }
            match individual {
                None => {
                    let left = self.viewer.viewport_mut();
                    if inward {
                        left.zoom_in();
                    } else {
                        left.zoom_out();
                    }
                    if inward {
                        state.viewport.zoom_in();
                    } else {
                        state.viewport.zoom_out();
                    }
                }
                Some(true) => {
                    if inward {
                        state.viewport.zoom_in();
                    } else {
                        state.viewport.zoom_out();
                    }
                }
                Some(false) => {
                    let left = self.viewer.viewport_mut();
                    if inward {
                        left.zoom_in();
                    } else {
                        left.zoom_out();
                    }
                }
            }
        }
        // After prepare_for_step both viewports render at their zoom field.
        let new_scale = if target_right {
            self.comparison
                .as_ref()
                .map_or(right_pre.zoom, |state| state.viewport.zoom)
        } else {
            self.viewer.viewport().zoom
        };

        let old_scale = f32::from(image_bounds.size.width) / dimensions.0 as f32;
        if !(usable_scale(old_scale) && usable_scale(new_scale)) {
            cx.notify();
            return;
        }
        let pre = if target_right { right_pre } else { left_pre };
        let (pan_x, pan_y) = anchored_pan(pre, dimensions, old_scale, new_scale, fraction);

        let Some(state) = self.comparison.as_mut() else {
            return;
        };
        match individual {
            None => {
                // Shift the partner viewport by the same delta so paired
                // images keep their alignment.
                let dx = pan_x - pre.pan_x;
                let dy = pan_y - pre.pan_y;
                if target_right {
                    state.viewport.set_pan(pan_x, pan_y);
                    self.viewer.viewport_mut().pan_by(dx, dy);
                } else {
                    self.viewer.viewport_mut().set_pan(pan_x, pan_y);
                    state.viewport.pan_by(dx, dy);
                }
            }
            Some(true) => state.viewport.set_pan(pan_x, pan_y),
            Some(false) => self.viewer.viewport_mut().set_pan(pan_x, pan_y),
        }
        cx.notify();
    }

    pub(crate) fn compare_pan(&mut self, dx: f32, dy: f32, cx: &mut Context<Self>) {
        if let Some(state) = self.comparison.as_mut() {
            match state.individual_target {
                None => {
                    self.viewer.viewport_mut().pan_by(dx, dy);
                    state.viewport.pan_by(dx, dy);
                }
                Some(true) => state.viewport.pan_by(dx, dy),
                Some(false) => self.viewer.viewport_mut().pan_by(dx, dy),
            }
            cx.notify();
        }
    }

    /// Rotate whichever panes comparison input currently targets.
    ///
    /// The left pane renders from the main viewer, so rotating it has to do
    /// everything `rotate_display` does — including rebuilding the main
    /// viewer's rotated bitmap. Without that rebuild `display_image()` finds
    /// no rotated copy and the left pane falls back to "Loading image...".
    pub(crate) fn compare_rotate(
        &mut self,
        quarter_turns: u8,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(individual) = self
            .comparison
            .as_ref()
            .map(|state| state.individual_target)
        else {
            return;
        };
        let affects_left = matches!(individual, None | Some(false));
        let affects_right = matches!(individual, None | Some(true));

        if affects_left {
            let previous_dimensions = self.viewer.display_dimensions();
            self.viewer.rotate_by(quarter_turns);
            if let Some((width, height)) = previous_dimensions {
                self.annotations.rotate_by(quarter_turns, width, height);
            }
        }
        if let Some(state) = self.comparison.as_mut() {
            if affects_right {
                state.rotation_quarter_turns = (state.rotation_quarter_turns + quarter_turns) % 4;
                state.rebuild_rotated_image();
            }
        }
        if affects_left {
            self.rebuild_rotated_image(Some(window), cx);
            self.refresh_large_image_tiles(window, cx);
        }
        self.ui.show_zoom_menu = false;
        cx.notify();
    }
}

fn load_decoded_image_from_path_with_policy(
    path: &PathBuf,
) -> Result<lumia_core::DecodedImage, lumia_core::ImageLoadError> {
    lumia_core::load_decoded_image_from_path(path)
}
