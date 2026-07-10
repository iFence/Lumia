use gpui::{Context, KeyBinding, Window};
use lumia_core::{default_shortcuts, Language, ShortcutId, ThemeAccent, ThemeMode};

use crate::app::LumiaApp;
use crate::persistence::save_settings;
use crate::{
    ExitFullscreen, NextImage, OpenFile, PreviousImage, Quit, SelectLanguage, SelectThemeAccent,
    SelectThemeMode, ToggleFullscreen, ToggleImageInfo, ZoomFit, ZoomIn, ZoomOut,
};

impl LumiaApp {
    pub(crate) fn set_language(&mut self, language: Language, cx: &mut Context<Self>) {
        self.settings.language = language;
        let _ = save_settings(&self.settings);
        cx.notify();
    }

    pub(crate) fn set_theme(&mut self, theme: ThemeMode, cx: &mut Context<Self>) {
        self.settings.theme = theme;
        let _ = save_settings(&self.settings);
        cx.notify();
    }

    pub(crate) fn set_theme_accent(&mut self, accent: ThemeAccent, cx: &mut Context<Self>) {
        self.settings.theme_accent = accent;
        let _ = save_settings(&self.settings);
        cx.notify();
    }

    pub(crate) fn apply_selected_language(
        &mut self,
        action: &SelectLanguage,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_language(action.0, cx);
    }

    pub(crate) fn apply_selected_theme_mode(
        &mut self,
        action: &SelectThemeMode,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_theme(action.0, cx);
    }

    pub(crate) fn apply_selected_theme_accent(
        &mut self,
        action: &SelectThemeAccent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.set_theme_accent(action.0, cx);
    }

    pub(crate) fn rebuild_keybindings(&self, cx: &mut Context<Self>) {
        cx.clear_key_bindings();
        let shortcuts = &self.settings.shortcuts;
        let mut bindings = Vec::new();

        if let Some(key) = shortcuts.get(&ShortcutId::OpenFile) {
            bindings.push(KeyBinding::new(key.as_str(), OpenFile, None));
        }
        if let Some(key) = shortcuts.get(&ShortcutId::ZoomIn) {
            bindings.push(KeyBinding::new(key.as_str(), ZoomIn, None));
        }
        if let Some(key) = shortcuts.get(&ShortcutId::ZoomOut) {
            bindings.push(KeyBinding::new(key.as_str(), ZoomOut, None));
        }
        if let Some(key) = shortcuts.get(&ShortcutId::ZoomFit) {
            bindings.push(KeyBinding::new(key.as_str(), ZoomFit, None));
        }
        if let Some(key) = shortcuts.get(&ShortcutId::ToggleFullscreen) {
            bindings.push(KeyBinding::new(key.as_str(), ToggleFullscreen, None));
        }
        if let Some(key) = shortcuts.get(&ShortcutId::ExitFullscreen) {
            bindings.push(KeyBinding::new(key.as_str(), ExitFullscreen, None));
        }
        if let Some(key) = shortcuts.get(&ShortcutId::ToggleImageInfo) {
            bindings.push(KeyBinding::new(key.as_str(), ToggleImageInfo, None));
        }
        if let Some(key) = shortcuts.get(&ShortcutId::NextImage) {
            bindings.push(KeyBinding::new(key.as_str(), NextImage, None));
        }
        if let Some(key) = shortcuts.get(&ShortcutId::PreviousImage) {
            bindings.push(KeyBinding::new(key.as_str(), PreviousImage, None));
        }
        if let Some(key) = shortcuts.get(&ShortcutId::Quit) {
            bindings.push(KeyBinding::new(key.as_str(), Quit, None));
        }
        cx.bind_keys(bindings);
    }

    pub(crate) fn start_recording_shortcut(
        &mut self,
        shortcut_id: ShortcutId,
        cx: &mut Context<Self>,
    ) {
        self.ui.recording_shortcut = Some(shortcut_id);
        cx.notify();
    }

    pub(crate) fn stop_recording_shortcut(&mut self, cx: &mut Context<Self>) {
        self.ui.recording_shortcut = None;
        cx.notify();
    }

    pub(crate) fn handle_shortcut_recording(
        &mut self,
        event: &gpui::KeyDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(shortcut_id) = self.ui.recording_shortcut else {
            return;
        };
        if event.keystroke.key == "escape" {
            self.stop_recording_shortcut(cx);
            return;
        }

        let binding = event.keystroke.unparse();
        if binding.is_empty() || ["shift", "ctrl", "alt", "cmd"].contains(&binding.as_str()) {
            return;
        }
        cx.stop_propagation();
        self.settings.shortcuts.insert(shortcut_id, binding);
        let _ = save_settings(&self.settings);
        self.rebuild_keybindings(cx);
        self.stop_recording_shortcut(cx);
    }

    pub(crate) fn reset_shortcut(&mut self, shortcut_id: ShortcutId, cx: &mut Context<Self>) {
        if let Some(binding) = default_shortcuts().get(&shortcut_id) {
            self.settings.shortcuts.insert(shortcut_id, binding.clone());
        } else {
            self.settings.shortcuts.remove(&shortcut_id);
        }
        let _ = save_settings(&self.settings);
        self.rebuild_keybindings(cx);
        cx.notify();
    }

    pub(crate) fn reset_all_shortcuts(&mut self, cx: &mut Context<Self>) {
        self.settings.shortcuts = default_shortcuts();
        let _ = save_settings(&self.settings);
        self.rebuild_keybindings(cx);
        cx.notify();
    }

    pub(crate) fn get_shortcut_binding(&self, shortcut_id: ShortcutId) -> String {
        self.settings
            .shortcuts
            .get(&shortcut_id)
            .cloned()
            .unwrap_or_default()
    }
}
