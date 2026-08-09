//! Lifecycle of the annotation text tool's input field.
//!
//! The plugin's declarative panel can only display a `TextInput`; it cannot own
//! an editable buffer that survives re-renders. So the host owns the
//! `InputState` entity, seeds it from the panel value on creation, and commits
//! the pending click point when the user presses Enter.

use gpui::{AppContext, Context, Entity, Window};
use gpui_component::input::{InputEvent, InputState};
use lumia_core::Annotation;
use lumia_plugin_api::CanvasOperation;

use crate::app::LumiaApp;
use crate::plugin_state::ActiveToolSettings;

impl LumiaApp {
    /// Resets any in-progress annotation interaction and drops the text input.
    pub(crate) fn clear_transient_annotation_ui(&mut self, cx: &mut Context<Self>) {
        self.ui.annotation_drag = None;
        self.ui.pending_text_point = None;
        self.annotation_text_input = None;
        self.annotation_text_input_subscription = None;
        cx.notify();
    }

    /// Keeps the host-owned text input in sync with the active tool: creates it
    /// once while the text tool is active, drops it otherwise. A live entity is
    /// never overwritten by a stale panel value.
    pub(crate) fn sync_annotation_text_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.plugins.is_text_tool_active() {
            if self.annotation_text_input.is_none() {
                let input = cx.new(|cx| InputState::new(window, cx));
                let subscription =
                    cx.subscribe_in(&input, window, Self::handle_annotation_text_input);
                self.annotation_text_input = Some(input);
                self.annotation_text_input_subscription = Some(subscription);
            }
        } else if self.annotation_text_input.is_some() {
            self.annotation_text_input = None;
            self.annotation_text_input_subscription = None;
        }
    }

    fn handle_annotation_text_input(
        &mut self,
        _input: &Entity<InputState>,
        event: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let InputEvent::PressEnter { shift: false, .. } = event {
            self.commit_pending_text(window, cx);
        }
    }

    /// Places the pending text annotation at the click point using the current
    /// input value, then clears the input for the next annotation.
    pub(crate) fn commit_pending_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((x, y)) = self.ui.pending_text_point else {
            return;
        };
        let text = self
            .annotation_text_input
            .as_ref()
            .map(|input| input.read(cx).value().trim().to_owned())
            .unwrap_or_default();
        if text.is_empty() {
            self.ui.pending_text_point = None;
            self.reset_annotation_text_input(window, cx);
            cx.notify();
            return;
        }
        let Some(ActiveToolSettings::Text {
            font_size,
            color,
            opacity,
        }) = self.plugins.active_tool_settings()
        else {
            return;
        };
        self.annotations.place(Annotation::Text {
            text: text.clone(),
            x,
            y,
            font_size,
            color,
            opacity,
        });
        self.notify_canvas_operation(
            CanvasOperation::TextPlaced {
                text,
                x,
                y,
                font_size,
                color: format!("#{:06x}", color),
                opacity,
            },
            cx,
        );
        self.ui.pending_text_point = None;
        self.reset_annotation_text_input(window, cx);
        cx.notify();
    }

    fn reset_annotation_text_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = self.annotation_text_input.clone() {
            input.update(cx, |state, cx| state.set_value(String::new(), window, cx));
        }
    }
}
