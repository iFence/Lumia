use std::time::Duration;

use gpui::{Context, Window};

use crate::app::LumiaApp;

const SLIDESHOW_INTERVAL: Duration = Duration::from_secs(3);

#[derive(Default)]
pub(crate) struct SlideshowState {
    active: bool,
    generation: u64,
}

impl SlideshowState {
    pub(crate) fn is_active(&self) -> bool {
        self.active
    }

    fn start(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.active = true;
        self.generation
    }

    fn stop(&mut self) -> bool {
        if !self.active {
            return false;
        }
        self.active = false;
        self.generation = self.generation.wrapping_add(1);
        true
    }

    fn is_current(&self, generation: u64) -> bool {
        self.active && self.generation == generation
    }
}

impl LumiaApp {
    pub(crate) fn can_start_slideshow(&self) -> bool {
        self.current_image_index().is_some() && self.sibling_count() > 1
    }

    pub(crate) fn toggle_slideshow(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.slideshow.is_active() {
            self.stop_slideshow(cx);
            return;
        }
        if !self.can_start_slideshow() {
            self.ui.context_menu_position = None;
            cx.notify();
            return;
        }

        self.close_plugin_session(cx);
        let generation = self.slideshow.start();
        self.ui.context_menu_position = None;
        cx.notify();

        cx.spawn_in(window, async move |this, cx| loop {
            cx.background_executor().timer(SLIDESHOW_INTERVAL).await;
            let should_continue = this
                .update_in(cx, |this, window, cx| {
                    if !this.slideshow.is_current(generation) {
                        return false;
                    }
                    if this.is_viewer_blocked() || !this.can_start_slideshow() {
                        this.stop_slideshow(cx);
                        return false;
                    }
                    if !this.loads.is_decoding() {
                        this.navigate_image(1, window, cx);
                        cx.notify();
                    }
                    true
                })
                .unwrap_or(false);
            if !should_continue {
                break;
            }
        })
        .detach();
    }

    pub(crate) fn stop_slideshow(&mut self, cx: &mut Context<Self>) -> bool {
        let stopped = self.slideshow.stop();
        if stopped {
            self.ui.context_menu_position = None;
            cx.notify();
        }
        stopped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restarting_invalidates_an_older_slideshow_loop() {
        let mut state = SlideshowState::default();
        let first_generation = state.start();
        assert!(state.is_current(first_generation));

        assert!(state.stop());
        let second_generation = state.start();

        assert!(!state.is_current(first_generation));
        assert!(state.is_current(second_generation));
    }

    #[test]
    fn stopping_an_inactive_slideshow_is_a_no_op() {
        let mut state = SlideshowState::default();
        assert!(!state.stop());
        assert!(!state.is_active());
    }
}
