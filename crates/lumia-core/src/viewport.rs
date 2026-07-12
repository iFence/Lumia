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
}
