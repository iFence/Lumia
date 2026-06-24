use gpui::{
    App, Context, FocusHandle, Focusable, KeyBinding, MouseMoveEvent, Subscription, Window,
};
use lumia_core::{
    default_shortcuts, supported_image_extensions, AppSettings, ImageDocument, ImageSource,
    Language, SettingsGroup, ShortcutId, ThemeMode, ViewportState,
};
use std::path::{Path, PathBuf};

use crate::persistence::{load_settings, save_settings};
use crate::util::format_load_error;
use crate::{
    ExitFullscreen, NextImage, OpenFile, PreviousImage, Quit, ToggleFullscreen, ToggleImageInfo,
    ZoomFit, ZoomIn, ZoomOut, APP_TITLE, TOOLBAR_HEIGHT, TITLE_BAR_HEIGHT,
};

pub(crate) struct LumiaApp {
    pub(crate) focus_handle: FocusHandle,
    pub(crate) viewport: ViewportState,
    pub(crate) current_image: Option<ImageDocument>,
    pub(crate) image_list: Vec<PathBuf>,
    pub(crate) error_message: Option<String>,
    pub(crate) pending_drop_paths: Vec<PathBuf>,
    pub(crate) is_panning: bool,
    pub(crate) is_fullscreen: bool,
    pub(crate) show_image_info: bool,
    pub(crate) context_menu_position: Option<gpui::Point<gpui::Pixels>>,
    pub(crate) last_mouse_position: Option<gpui::Point<gpui::Pixels>>,
    pub(crate) show_settings_panel: bool,
    pub(crate) active_settings_group: SettingsGroup,
    pub(crate) settings: AppSettings,
    pub(crate) appearance_subscription: Option<Subscription>,
    pub(crate) toolbar_locked: bool,
    pub(crate) root_mouse_y: f32,
    pub(crate) recording_shortcut: Option<ShortcutId>,
    pub(crate) window_title: String,
    /// Whether a background decode is in progress for a HEIC/HEIF image.
    pub(crate) is_decoding: bool,
    /// Receiver for the background HEIC→PNG decode result.
    /// Polled non-blockingly in `render_viewer`.
    pub(crate) pending_decode: Option<std::sync::mpsc::Receiver<Option<lumia_core::CachedImage>>>,
}

impl LumiaApp {
    pub(crate) fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        initial_path: Option<PathBuf>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle);
        let settings = load_settings();

        let mut app = Self {
            focus_handle,
            viewport: ViewportState::default(),
            current_image: None,
            image_list: Vec::new(),
            error_message: None,
            pending_drop_paths: Vec::new(),
            is_panning: false,
            is_fullscreen: false,
            show_image_info: false,
            context_menu_position: None,
            last_mouse_position: None,
            show_settings_panel: false,
            active_settings_group: SettingsGroup::General,
            settings,
            appearance_subscription: None,
            toolbar_locked: false,
            root_mouse_y: 0.0,
            recording_shortcut: None,
            window_title: APP_TITLE.to_string(),
            is_decoding: false,
            pending_decode: None,
        };
        app.appearance_subscription = Some(cx.observe_window_appearance(window, |_, _, cx| {
            cx.notify();
        }));
        app.rebuild_keybindings(cx);
        window.toggle_fullscreen();
        app.is_fullscreen = true;

        // Load the image if the app was launched via OS file-open or CLI argument.
        if let Some(path) = initial_path {
            app.load_image(path, Some(window), cx);
        }

        app
    }

    /// When the settings panel is visible, block actions that operate on the viewer.
    fn is_viewer_blocked(&self) -> bool {
        self.show_settings_panel
    }

    pub(crate) fn open_file(&mut self, _: &OpenFile, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_viewer_blocked() {
            return;
        }
        self.open_file_dialog(cx, Some(window));
    }

    pub(crate) fn open_file_dialog(&mut self, cx: &mut Context<Self>, window: Option<&mut Window>) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Images", supported_image_extensions())
            .pick_file()
        {
            self.load_image(path, window, cx);
            cx.notify();
        }
    }

    pub(crate) fn zoom_in(&mut self, _: &ZoomIn, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_viewer_blocked() {
            return;
        }
        self.viewport.zoom_in();
        cx.notify();
    }

    pub(crate) fn zoom_out(&mut self, _: &ZoomOut, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_viewer_blocked() {
            return;
        }
        self.viewport.zoom_out();
        cx.notify();
    }

    pub(crate) fn zoom_fit(&mut self, _: &ZoomFit, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_viewer_blocked() {
            return;
        }
        self.reset_fit(cx);
    }

    pub(crate) fn reset_fit(&mut self, cx: &mut Context<Self>) {
        self.viewport.reset_fit();
        cx.notify();
    }

    pub(crate) fn toggle_fullscreen(
        &mut self,
        _: &ToggleFullscreen,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_viewer_blocked() {
            return;
        }
        self.toggle_window_fullscreen(window, cx);
    }

    pub(crate) fn toggle_window_fullscreen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.toggle_fullscreen();
        self.is_fullscreen = !self.is_fullscreen;
        cx.notify();
    }

    pub(crate) fn exit_fullscreen(
        &mut self,
        _: &ExitFullscreen,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_viewer_blocked() {
            return;
        }
        if self.is_fullscreen || window.is_fullscreen() {
            window.toggle_fullscreen();
            self.is_fullscreen = false;
            cx.notify();
        }
    }

    pub(crate) fn toggle_image_info(
        &mut self,
        _: &ToggleImageInfo,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_viewer_blocked() {
            return;
        }
        self.show_image_info = !self.show_image_info;
        cx.notify();
    }

    pub(crate) fn open_settings_panel(&mut self, cx: &mut Context<Self>) {
        self.show_settings_panel = true;
        self.active_settings_group = SettingsGroup::General;
        self.context_menu_position = None;
        cx.notify();
    }

    pub(crate) fn close_settings_panel(&mut self, cx: &mut Context<Self>) {
        self.show_settings_panel = false;
        self.recording_shortcut = None;
        cx.notify();
    }

    pub(crate) fn select_settings_group(&mut self, group: SettingsGroup, cx: &mut Context<Self>) {
        self.active_settings_group = group;
        cx.notify();
    }

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

    /// Rebuild all keybindings from settings (or defaults).
    pub(crate) fn rebuild_keybindings(&self, cx: &mut Context<Self>) {
        cx.clear_key_bindings();
        let shortcuts = &self.settings.shortcuts;

        let mut bindings: Vec<KeyBinding> = Vec::new();

        if let Some(ks) = shortcuts.get(&ShortcutId::OpenFile) {
            bindings.push(KeyBinding::new(ks.as_str(), OpenFile, None));
        }
        if let Some(ks) = shortcuts.get(&ShortcutId::ZoomIn) {
            bindings.push(KeyBinding::new(ks.as_str(), ZoomIn, None));
        }
        if let Some(ks) = shortcuts.get(&ShortcutId::ZoomOut) {
            bindings.push(KeyBinding::new(ks.as_str(), ZoomOut, None));
        }
        if let Some(ks) = shortcuts.get(&ShortcutId::ZoomFit) {
            bindings.push(KeyBinding::new(ks.as_str(), ZoomFit, None));
        }
        if let Some(ks) = shortcuts.get(&ShortcutId::ToggleFullscreen) {
            bindings.push(KeyBinding::new(ks.as_str(), ToggleFullscreen, None));
        }
        if let Some(ks) = shortcuts.get(&ShortcutId::ExitFullscreen) {
            bindings.push(KeyBinding::new(ks.as_str(), ExitFullscreen, None));
        }
        if let Some(ks) = shortcuts.get(&ShortcutId::ToggleImageInfo) {
            bindings.push(KeyBinding::new(ks.as_str(), ToggleImageInfo, None));
        }
        if let Some(ks) = shortcuts.get(&ShortcutId::NextImage) {
            bindings.push(KeyBinding::new(ks.as_str(), NextImage, None));
        }
        if let Some(ks) = shortcuts.get(&ShortcutId::PreviousImage) {
            bindings.push(KeyBinding::new(ks.as_str(), PreviousImage, None));
        }
        if let Some(ks) = shortcuts.get(&ShortcutId::Quit) {
            bindings.push(KeyBinding::new(ks.as_str(), Quit, None));
        }

        cx.bind_keys(bindings);
    }

    pub(crate) fn start_recording_shortcut(
        &mut self,
        shortcut_id: ShortcutId,
        cx: &mut Context<Self>,
    ) {
        self.recording_shortcut = Some(shortcut_id);
        cx.notify();
    }

    pub(crate) fn stop_recording_shortcut(&mut self, cx: &mut Context<Self>) {
        self.recording_shortcut = None;
        cx.notify();
    }

    pub(crate) fn handle_shortcut_recording(
        &mut self,
        event: &gpui::KeyDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(shortcut_id) = self.recording_shortcut else {
            return;
        };

        // Escape cancels recording without changing the binding
        if event.keystroke.key == "escape" {
            self.stop_recording_shortcut(cx);
            return;
        }

        let binding_string = event.keystroke.unparse();

        // Ignore standalone modifier keypresses
        if binding_string.is_empty()
            || binding_string == "shift"
            || binding_string == "ctrl"
            || binding_string == "alt"
            || binding_string == "cmd"
        {
            return;
        }

        // Stop propagation so the keystroke doesn't also fire its action
        cx.stop_propagation();

        self.settings
            .shortcuts
            .insert(shortcut_id, binding_string);
        let _ = save_settings(&self.settings);
        self.rebuild_keybindings(cx);
        self.stop_recording_shortcut(cx);
    }

    pub(crate) fn reset_shortcut(&mut self, shortcut_id: ShortcutId, cx: &mut Context<Self>) {
        let defaults = default_shortcuts();
        if let Some(default_binding) = defaults.get(&shortcut_id) {
            self.settings
                .shortcuts
                .insert(shortcut_id, default_binding.clone());
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

    pub(crate) fn load_image(
        &mut self,
        path: PathBuf,
        window: Option<&mut Window>,
        _cx: &mut Context<Self>,
    ) {
        match ImageDocument::load_from_path(&path) {
            Ok(document) => {
                let needs_async_decode = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|ext| {
                        ext.eq_ignore_ascii_case("heic") || ext.eq_ignore_ascii_case("heif")
                    });

                self.current_image = Some(document);
                self.error_message = None;
                self.viewport.reset_fit();
                self.is_panning = false;
                self.context_menu_position = None;
                self.last_mouse_position = None;
                self.scan_sibling_images();
                self.window_title = self.image_name();
                if let Some(window) = window {
                    window.set_window_title(&self.window_title);
                }

                if needs_async_decode {
                    self.is_decoding = true;

                    // Decode HEIC → PNG on a background thread so the UI
                    // stays responsive. `render_viewer` polls the receiver
                    // non-blockingly to pick up the result when ready.
                    let file_path = path.clone();
                    let (tx, rx) = std::sync::mpsc::channel();
                    self.pending_decode = Some(rx);
                    std::thread::spawn(move || {
                        let cached = std::fs::read(&file_path)
                            .ok()
                            .and_then(|bytes| lumia_core::decode_heic_to_png(&bytes).ok());
                        let _ = tx.send(cached);
                    });
                }
            }
            Err(error) => {
                self.error_message = Some(format_load_error(&error));
                self.is_panning = false;
                self.context_menu_position = None;
                self.last_mouse_position = None;
                self.is_decoding = false;
            }
        }
    }

    fn scan_sibling_images(&mut self) {
        self.image_list.clear();
        let Some(current_path) = self.image_path() else {
            return;
        };
        let Some(parent_dir) = current_path.parent() else {
            return;
        };
        let mut entries: Vec<PathBuf> = match std::fs::read_dir(parent_dir) {
            Ok(entries) => entries
                .filter_map(|entry| entry.ok().map(|e| e.path()))
                .filter(|path| {
                    path.extension()
                        .and_then(|ext| ext.to_str())
                        .is_some_and(lumia_core::is_supported_image_extension)
                })
                .collect(),
            Err(_) => return,
        };
        entries.sort_by(|a, b| {
            a.file_name()
                .and_then(|n| n.to_str())
                .cmp(&b.file_name().and_then(|n| n.to_str()))
        });
        self.image_list = entries;
    }

    pub(crate) fn current_image_index(&self) -> Option<usize> {
        let current_path = self.image_path()?;
        self.image_list.iter().position(|p| p == current_path)
    }

    pub(crate) fn sibling_count(&self) -> usize {
        self.image_list.len()
    }

    pub(crate) fn navigate_image(
        &mut self,
        step: i32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(current_idx) = self.current_image_index() else {
            return;
        };
        let new_idx = if step < 0 {
            current_idx.saturating_sub(step.unsigned_abs() as usize)
        } else {
            let next = current_idx + step as usize;
            if next >= self.image_list.len() {
                self.image_list.len().saturating_sub(1)
            } else {
                next
            }
        };
        if new_idx != current_idx && new_idx < self.image_list.len() {
            let path = self.image_list[new_idx].clone();
            self.load_image(path, Some(window), cx);
        }
    }

    pub(crate) fn next_image(&mut self, _: &NextImage, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_viewer_blocked() {
            return;
        }
        self.navigate_image(1, window, cx);
        cx.notify();
    }

    pub(crate) fn previous_image(
        &mut self,
        _: &PreviousImage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_viewer_blocked() {
            return;
        }
        self.navigate_image(-1, window, cx);
        cx.notify();
    }

    pub(crate) fn load_first_supported_drop(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let path = self
            .pending_drop_paths
            .iter()
            .find(|path| {
                path.extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(lumia_core::is_supported_image_extension)
            })
            .cloned();

        match path {
            Some(path) => self.load_image(path, Some(window), cx),
            None => {
                self.error_message = Some("No supported image found in dropped files".to_string());
            }
        }
        self.pending_drop_paths.clear();
    }

    pub(crate) fn image_path(&self) -> Option<&Path> {
        match self.current_image.as_ref().map(|document| &document.source) {
            Some(ImageSource::LocalPath(path) | ImageSource::TemporaryPath(path)) => Some(path),
            None => None,
        }
    }

    pub(crate) fn image_name(&self) -> String {
        self.image_path()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("No image")
            .to_string()
    }

    pub(crate) fn scaled_image_size(&self, window: &Window) -> Option<(f32, f32)> {
        let metadata = self.current_image.as_ref()?.metadata.as_ref()?;
        let viewport_size = window.viewport_size();
        let available_width = f32::from(viewport_size.width).max(1.0);
        let chrome_height = if self.is_fullscreen {
            if self.should_show_toolbar() {
                TITLE_BAR_HEIGHT + TOOLBAR_HEIGHT
            } else {
                0.0
            }
        } else {
            TOOLBAR_HEIGHT
        };
        let available_height = (f32::from(viewport_size.height) - chrome_height).max(1.0);
        let image_width = metadata.width as f32;
        let image_height = metadata.height as f32;
        let fit_scale = (available_width / image_width)
            .min(available_height / image_height)
            .min(1.0);
        let scale = fit_scale * self.viewport.zoom;

        Some((image_width * scale, image_height * scale))
    }

    const HOVER_ZONE_HEIGHT: f32 = 72.0;

    pub(crate) fn should_show_toolbar(&self) -> bool {
        !self.is_fullscreen || self.toolbar_locked || self.root_mouse_y <= Self::HOVER_ZONE_HEIGHT
    }

    pub(crate) fn should_show_titlebar(&self) -> bool {
        self.is_fullscreen && self.should_show_toolbar()
    }

    pub(crate) fn toggle_toolbar_lock(&mut self, cx: &mut Context<Self>) {
        self.toolbar_locked = !self.toolbar_locked;
        cx.notify();
    }

    pub(crate) fn handle_root_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let new_y = f32::from(event.position.y);
        let was_in_zone = self.root_mouse_y <= Self::HOVER_ZONE_HEIGHT;
        let is_in_zone = new_y <= Self::HOVER_ZONE_HEIGHT;
        self.root_mouse_y = new_y;
        if was_in_zone != is_in_zone {
            cx.notify();
        }
    }
}

impl Focusable for LumiaApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
