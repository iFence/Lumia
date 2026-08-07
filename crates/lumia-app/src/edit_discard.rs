use gpui::{Context, Window};
use gpui_component::dialog::DialogButtonProps;
use gpui_component::WindowExt;
use lumia_core::{CropRect, Language};

use crate::app::LumiaApp;
use crate::editing::{CropAspect, EditMode, EditState};
use crate::i18n::{tr, TextKey};

/// The edit action the user requested after confirming to discard the
/// current, unapplied edit session changes.
#[derive(Debug, Clone, Copy)]
enum DiscardAction {
    SwitchMode(EditMode),
    Close,
}

/// Whether the active edit session carries unapplied changes that would be
/// lost if the user switches modes or closes the edit panel.
fn edit_session_has_changes(state: &EditState) -> bool {
    match state.mode {
        Some(EditMode::Crop) => {
            state.crop_aspect != CropAspect::Free
                || state.crop_rect != CropRect::new(0, 0, state.source_width, state.source_height)
        }
        Some(EditMode::Resize) => {
            state.resize_width != state.source_width
                || state.resize_height != state.source_height
                || !state.lock_aspect
        }
        None => false,
    }
}

/// Localized strings for the discard-confirmation dialog. Kept in this module
/// because `i18n.rs` sits exactly at the architecture test's 500-line ceiling
/// and has no room for new keys without a dedicated split.
fn discard_texts(language: Language) -> (&'static str, &'static str, &'static str) {
    match language {
        Language::English => (
            "Discard changes?",
            "This will discard your current crop or resize changes.",
            "Discard",
        ),
        Language::Chinese => (
            "放弃当前修改？",
            "此操作将放弃当前的裁剪或尺寸修改。",
            "放弃修改",
        ),
    }
}

impl LumiaApp {
    /// Closes the edit session, confirming with the user first when the
    /// current edits have not been applied. Used by the panel's close and
    /// cancel controls; navigation keeps calling `close_edit_session` directly.
    pub(crate) fn request_close_edit_session(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if edit_session_has_changes(&self.editing) {
            self.prompt_discard_edit_changes(DiscardAction::Close, window, cx);
            return;
        }
        self.close_edit_session(true, cx);
    }

    /// Switches the edit mode, confirming with the user first when the current
    /// edits have not been applied. Called by `open_edit_mode`; the dialog is
    /// modal, so the edit state cannot change while it is open.
    pub(crate) fn request_edit_mode_switch(
        &mut self,
        mode: EditMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if edit_session_has_changes(&self.editing) {
            self.editing.show_menu = false;
            self.ui.context_menu_position = None;
            cx.notify();
            self.prompt_discard_edit_changes(DiscardAction::SwitchMode(mode), window, cx);
            return;
        }
        self.enter_edit_mode(mode, window, cx);
    }

    fn prompt_discard_edit_changes(
        &self,
        action: DiscardAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let language = self.settings.language;
        let (title, message, ok_text) = discard_texts(language);
        let self_handle = self.self_handle.clone();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let self_handle = self_handle.clone();
            alert
                .confirm()
                .title(title)
                .description(message)
                .button_props(
                    DialogButtonProps::default()
                        .show_cancel(true)
                        .ok_text(ok_text)
                        .cancel_text(tr(language, TextKey::Cancel)),
                )
                .on_ok(move |_, window, cx| {
                    let _ = self_handle.update(cx, |this, cx| match action {
                        DiscardAction::SwitchMode(mode) => this.enter_edit_mode(mode, window, cx),
                        DiscardAction::Close => this.close_edit_session(true, cx),
                    });
                    true
                })
                .on_cancel(|_, _, _| true)
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_session_with_full_selection_has_no_changes() {
        let state = EditState {
            mode: Some(EditMode::Crop),
            source_width: 100,
            source_height: 80,
            crop_rect: CropRect::new(0, 0, 100, 80),
            ..EditState::default()
        };
        assert!(!edit_session_has_changes(&state));
    }

    #[test]
    fn moved_crop_selection_has_changes() {
        let state = EditState {
            mode: Some(EditMode::Crop),
            source_width: 100,
            source_height: 80,
            crop_rect: CropRect::new(5, 5, 90, 70),
            ..EditState::default()
        };
        assert!(edit_session_has_changes(&state));
    }

    #[test]
    fn changed_crop_aspect_has_changes() {
        let state = EditState {
            mode: Some(EditMode::Crop),
            source_width: 100,
            source_height: 80,
            crop_aspect: CropAspect::Square,
            ..EditState::default()
        };
        assert!(edit_session_has_changes(&state));
    }

    #[test]
    fn resize_session_at_original_size_has_no_changes() {
        let state = EditState {
            mode: Some(EditMode::Resize),
            source_width: 100,
            source_height: 80,
            resize_width: 100,
            resize_height: 80,
            lock_aspect: true,
            ..EditState::default()
        };
        assert!(!edit_session_has_changes(&state));
    }

    #[test]
    fn edited_resize_values_have_changes() {
        let state = EditState {
            mode: Some(EditMode::Resize),
            source_width: 100,
            source_height: 80,
            resize_width: 50,
            resize_height: 40,
            lock_aspect: true,
            ..EditState::default()
        };
        assert!(edit_session_has_changes(&state));
    }

    #[test]
    fn unlocked_aspect_has_changes() {
        let state = EditState {
            mode: Some(EditMode::Resize),
            source_width: 100,
            source_height: 80,
            resize_width: 100,
            resize_height: 80,
            lock_aspect: false,
            ..EditState::default()
        };
        assert!(edit_session_has_changes(&state));
    }

    #[test]
    fn no_session_has_no_changes() {
        assert!(!edit_session_has_changes(&EditState::default()));
    }
}
