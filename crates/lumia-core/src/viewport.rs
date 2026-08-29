use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ViewportState {
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
    pub fit_mode: FitMode,
}

impl Default for ViewportState {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            fit_mode: FitMode::FitToWindow,
        }
    }
}

impl ViewportState {
    pub const MIN_ZOOM: f32 = 0.1;
    pub const MAX_ZOOM: f32 = 32.0;
    pub const ZOOM_STEP: f32 = 1.2;

    pub fn zoom_in(&mut self) {
        self.zoom = (self.zoom * Self::ZOOM_STEP).min(Self::MAX_ZOOM);
        self.fit_mode = FitMode::ActualSize;
    }

    pub fn zoom_out(&mut self) {
        self.zoom = (self.zoom / Self::ZOOM_STEP).max(Self::MIN_ZOOM);
        self.fit_mode = FitMode::ActualSize;
    }

    pub fn set_zoom(&mut self, zoom: f32) {
        self.zoom = zoom.clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
        self.fit_mode = FitMode::ActualSize;
    }

    pub fn reset_fit(&mut self) {
        self.zoom = 1.0;
        self.pan_x = 0.0;
        self.pan_y = 0.0;
        self.fit_mode = FitMode::FitToWindow;
    }

    pub fn reset_actual_size(&mut self) {
        self.zoom = 1.0;
        self.pan_x = 0.0;
        self.pan_y = 0.0;
        self.fit_mode = FitMode::ActualSize;
    }

    pub fn pan_by(&mut self, dx: f32, dy: f32) {
        self.pan_x += dx;
        self.pan_y += dy;
    }

    pub fn set_pan(&mut self, x: f32, y: f32) {
        self.pan_x = x;
        self.pan_y = y;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FitMode {
    ActualSize,
    FitToWindow,
    FitWidth,
}

/// Pan adjustment that keeps the image point under `cursor` stationary while
/// the effective display scale changes from `old_scale` to `new_scale`.
///
/// The viewer layout must be a pane centered at `center` that shows the image
/// offset by `pan`, where the image occupies `display_size` screen pixels
/// before the zoom change. Returns the `(dx, dy)` delta to add to both pan
/// offsets of every viewport that must stay aligned with the anchored one.
pub fn anchor_pan_delta(
    old_scale: f32,
    new_scale: f32,
    cursor: (f32, f32),
    center: (f32, f32),
    pan: (f32, f32),
    display_size: (f32, f32),
) -> (f32, f32) {
    if old_scale <= 0.0 || new_scale <= 0.0 {
        return (0.0, 0.0);
    }
    let image_x = (cursor.0 - center.0 + display_size.0 / 2.0 - pan.0) / old_scale;
    let image_y = (cursor.1 - center.1 + display_size.1 / 2.0 - pan.1) / old_scale;
    let scale_delta = new_scale - old_scale;
    (
        scale_delta * (display_size.0 / 2.0 - image_x),
        scale_delta * (display_size.1 / 2.0 - image_y),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_operations_clamp_reset_and_accumulate() {
        let mut viewport = ViewportState::default();
        for _ in 0..64 {
            viewport.zoom_in();
        }
        assert_eq!(viewport.zoom, ViewportState::MAX_ZOOM);
        assert_eq!(viewport.fit_mode, FitMode::ActualSize);

        viewport.set_zoom(64.0);
        assert_eq!(viewport.zoom, ViewportState::MAX_ZOOM);

        viewport.set_zoom(0.01);
        assert_eq!(viewport.zoom, ViewportState::MIN_ZOOM);

        viewport.set_zoom(1.5);
        assert_eq!(viewport.zoom, 1.5);
        assert_eq!(viewport.fit_mode, FitMode::ActualSize);

        for _ in 0..128 {
            viewport.zoom_out();
        }
        assert_eq!(viewport.zoom, ViewportState::MIN_ZOOM);

        viewport.pan_by(12.0, -4.5);
        viewport.pan_by(-2.0, 1.5);
        assert_eq!(viewport.pan_x, 10.0);
        assert_eq!(viewport.pan_y, -3.0);

        viewport.set_pan(-8.0, 6.0);
        assert_eq!(viewport.pan_x, -8.0);
        assert_eq!(viewport.pan_y, 6.0);

        viewport.reset_fit();
        assert_eq!(viewport.zoom, 1.0);
        assert_eq!(viewport.pan_x, 0.0);
        assert_eq!(viewport.pan_y, 0.0);
        assert_eq!(viewport.fit_mode, FitMode::FitToWindow);

        viewport.pan_by(5.0, 8.0);
        viewport.reset_actual_size();
        assert_eq!(viewport.zoom, 1.0);
        assert_eq!(viewport.pan_x, 0.0);
        assert_eq!(viewport.pan_y, 0.0);
        assert_eq!(viewport.fit_mode, FitMode::ActualSize);
    }

    #[test]
    fn anchor_pan_delta_keeps_cursor_point_stationary() {
        // Pane centered at (400, 300); a 100x80 image shown at scale 1 with
        // no pan sits at (350, 260)..(450, 340).
        let center = (400.0, 300.0);
        let display = (100.0, 80.0);
        let pan = (12.0, -6.0);
        let cursor = (420.0, 280.0);

        for new_scale in [0.25_f32, 0.5, 1.5, 2.0, 4.0] {
            let (dx, dy) = anchor_pan_delta(1.0, new_scale, cursor, center, pan, display);
            let image_x = (cursor.0 - center.0 + display.0 / 2.0 - pan.0) / 1.0;
            let image_y = (cursor.1 - center.1 + display.1 / 2.0 - pan.1) / 1.0;
            let position_x =
                center.0 - display.0 * new_scale / 2.0 + (pan.0 + dx) + image_x * new_scale;
            let position_y =
                center.1 - display.1 * new_scale / 2.0 + (pan.1 + dy) + image_y * new_scale;
            assert!((position_x - cursor.0).abs() < 1e-4);
            assert!((position_y - cursor.1).abs() < 1e-4);
        }
    }

    #[test]
    fn anchor_pan_delta_ignores_degenerate_scales() {
        let delta = anchor_pan_delta(0.0, 2.0, (10.0, 10.0), (0.0, 0.0), (0.0, 0.0), (8.0, 8.0));
        assert_eq!(delta, (0.0, 0.0));
        let delta = anchor_pan_delta(1.0, -1.0, (10.0, 10.0), (0.0, 0.0), (0.0, 0.0), (8.0, 8.0));
        assert_eq!(delta, (0.0, 0.0));
    }
}
