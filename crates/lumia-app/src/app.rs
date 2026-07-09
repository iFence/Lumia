use gpui::{
    App, Context, FocusHandle, Focusable, KeyBinding, MouseMoveEvent, Subscription, WeakEntity,
    Window,
};
use lumia_core::{
    default_shortcuts, load_cached_image_from_path, rotate_cached_image,
    supported_image_extensions, AppSettings, CachedImage, FitMode, ImageDocument, ImageSource,
    Language, SettingsGroup, ShortcutId, ThemeAccent, ThemeMode, ViewportState,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::persistence::{load_settings, save_settings};
use crate::util::format_load_error;
use crate::{
    ExitFullscreen, NextImage, OpenFile, PreviousImage, Quit, RotateClockwise,
    RotateCounterClockwise, SelectLanguage, SelectThemeAccent, SelectThemeMode, ToggleFullscreen,
    ToggleImageInfo, ZoomFit, ZoomIn, ZoomOut, APP_TITLE, STATUS_BAR_HEIGHT, ZOOM_MENU_BOTTOM_GAP,
    ZOOM_MENU_HEIGHT, ZOOM_MENU_HOVER_MARGIN, ZOOM_MENU_RIGHT, ZOOM_MENU_WIDTH,
};

pub(crate) struct LumiaApp {
    pub(crate) self_handle: WeakEntity<LumiaApp>,
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
    pub(crate) recording_shortcut: Option<ShortcutId>,
    pub(crate) window_title: String,
    /// Whether a background decode is in progress for a HEIC/HEIF image.
    pub(crate) is_decoding: bool,
    /// Pre-decoded adjacent images for instant navigation.
    pub(crate) preload_cache: HashMap<PathBuf, CachedImage>,
    /// Receivers for in-flight preload decode tasks.
    pub(crate) pending_preloads: Vec<std::sync::mpsc::Receiver<Option<(PathBuf, CachedImage)>>>,
    pub(crate) rotation_quarter_turns: u8,
    pub(crate) rotated_image: Option<CachedImage>,
    pub(crate) show_zoom_menu: bool,
    pub(crate) show_status_bar: bool,
}

impl LumiaApp {
    pub(crate) fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        initial_path: Option<PathBuf>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        let settings = load_settings();

        let mut app = Self {
            self_handle: WeakEntity::new_invalid(),
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
            recording_shortcut: None,
            window_title: APP_TITLE.to_string(),
            is_decoding: false,
            preload_cache: HashMap::new(),
            pending_preloads: Vec::new(),
            rotation_quarter_turns: 0,
            rotated_image: None,
            show_zoom_menu: false,
            show_status_bar: false,
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

    pub(crate) fn set_self_handle(
        &mut self,
        self_handle: WeakEntity<LumiaApp>,
        cx: &mut Context<Self>,
    ) {
        self.self_handle = self_handle;
        cx.notify();
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
        self.show_zoom_menu = false;
        cx.notify();
    }

    pub(crate) fn zoom_out(&mut self, _: &ZoomOut, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_viewer_blocked() {
            return;
        }
        self.viewport.zoom_out();
        self.show_zoom_menu = false;
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
        self.show_zoom_menu = false;
        cx.notify();
    }

    pub(crate) fn set_zoom(&mut self, zoom: f32, cx: &mut Context<Self>) {
        if self.is_viewer_blocked() || self.current_image.is_none() {
            return;
        }
        self.viewport.set_zoom(zoom);
        self.show_zoom_menu = false;
        cx.notify();
    }

    pub(crate) fn toggle_fit_or_actual_size(&mut self, cx: &mut Context<Self>) {
        if self.is_viewer_blocked() || self.current_image.is_none() {
            return;
        }
        if self.viewport.fit_mode == FitMode::FitToWindow {
            self.viewport.set_zoom(1.0);
        } else {
            self.viewport.reset_fit();
        }
        self.show_zoom_menu = false;
        cx.notify();
    }

    pub(crate) fn toggle_zoom_menu(&mut self, cx: &mut Context<Self>) {
        if self.is_viewer_blocked() || self.current_image.is_none() {
            return;
        }
        self.show_zoom_menu = !self.show_zoom_menu;
        cx.notify();
    }

    pub(crate) fn rotate_clockwise(
        &mut self,
        _: &RotateClockwise,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.rotate_display(1, cx);
    }

    pub(crate) fn rotate_counter_clockwise(
        &mut self,
        _: &RotateCounterClockwise,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.rotate_display(3, cx);
    }

    pub(crate) fn rotate_display(&mut self, quarter_turns: u8, cx: &mut Context<Self>) {
        if self.is_viewer_blocked() || self.current_image.is_none() {
            return;
        }
        self.rotation_quarter_turns = (self.rotation_quarter_turns + quarter_turns) % 4;
        self.viewport.reset_fit();
        self.show_zoom_menu = false;
        self.rebuild_rotated_image();
        cx.notify();
    }

    pub(crate) fn rebuild_rotated_image(&mut self) {
        self.rotated_image = None;
        let turns = self.rotation_quarter_turns % 4;
        if turns == 0 {
            return;
        }

        let cached = self
            .current_image
            .as_ref()
            .and_then(|document| document.cached_image.clone())
            .or_else(|| {
                self.image_path()
                    .and_then(|path| load_cached_image_from_path(path).ok())
            });
        self.rotated_image = cached
            .as_ref()
            .and_then(|image| rotate_cached_image(image, turns).ok());
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

    pub(crate) fn set_theme_accent(&mut self, theme_accent: ThemeAccent, cx: &mut Context<Self>) {
        self.settings.theme_accent = theme_accent;
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

        self.settings.shortcuts.insert(shortcut_id, binding_string);
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
                let needs_async_decode =
                    path.extension()
                        .and_then(|e| e.to_str())
                        .is_some_and(|ext| {
                            ext.eq_ignore_ascii_case("heic") || ext.eq_ignore_ascii_case("heif")
                        });

                // Always discard any in-flight decode from the previously
                // displayed image — a new image is taking over.
                self.is_decoding = false;

                self.current_image = Some(document);
                self.error_message = None;
                self.viewport.reset_fit();
                self.rotation_quarter_turns = 0;
                self.rotated_image = None;
                self.show_zoom_menu = false;
                self.is_panning = false;
                self.context_menu_position = None;
                self.last_mouse_position = None;
                self.scan_sibling_images();
                self.window_title = self.image_name();
                if let Some(window) = window {
                    window.set_window_title(&self.window_title);
                }

                // If this image was preloaded in the background, apply the
                // cached image now so rendering is instant.
                if let Some(cached) = self.preload_cache.remove(&path) {
                    if let Some(ref mut doc) = self.current_image {
                        doc.cached_image = Some(cached);
                    }
                }

                if needs_async_decode
                    && self
                        .current_image
                        .as_ref()
                        .and_then(|doc| doc.cached_image.as_ref())
                        .is_none()
                {
                    self.is_decoding = true;

                    // Take the file bytes that load_from_path already read
                    // for metadata extraction. This avoids a second disk read.
                    let heif_bytes = self
                        .current_image
                        .as_mut()
                        .and_then(|doc| doc.heif_bytes.take());

                    let decode_path = path.clone();
                    _cx.spawn(async move |this, cx| {
                        let cached = cx
                            .background_executor()
                            .spawn(async move {
                                heif_bytes
                                    .or_else(|| {
                                        // Fallback: re-read from disk if bytes weren't
                                        // cached (shouldn't normally happen).
                                        std::fs::read(&decode_path).ok()
                                    })
                                    .and_then(|bytes| lumia_core::decode_heic_to_png(&bytes).ok())
                            })
                            .await;

                        let _ = this.update(cx, |this, cx| {
                            if this.image_path() != Some(path.as_path()) {
                                return;
                            }
                            if let Some(ref mut doc) = this.current_image {
                                doc.cached_image = cached;
                            }
                            if this.rotation_quarter_turns != 0 {
                                this.rebuild_rotated_image();
                            }
                            this.is_decoding = false;
                            cx.notify();
                        });
                    })
                    .detach();
                }

                // Preload adjacent images so next/previous navigation is instant.
                self.start_preload_adjacent();
            }
            Err(error) => {
                self.error_message = Some(format_load_error(&error));
                self.is_panning = false;
                self.context_menu_position = None;
                self.last_mouse_position = None;
                self.show_zoom_menu = false;
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

    /// Kick off background decodes for the immediately adjacent HEIC images
    /// so that next/previous navigation feels instant.
    fn start_preload_adjacent(&mut self) {
        let Some(current_idx) = self.current_image_index() else {
            return;
        };

        // Preload up to 2 images: previous and next.
        for offset in [-1i32, 1] {
            let target_idx = if offset < 0 {
                current_idx.saturating_sub(offset.unsigned_abs() as usize)
            } else {
                let next = current_idx + offset as usize;
                if next >= self.image_list.len() {
                    continue;
                }
                next
            };
            if target_idx == current_idx || target_idx >= self.image_list.len() {
                continue;
            }

            let target_path = self.image_list[target_idx].clone();

            // Skip if already cached or already queued.
            if self.preload_cache.contains_key(&target_path) {
                continue;
            }

            // Only preload HEIC/HEIF — other formats are fast-path via GPUI.
            let is_heic = target_path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|ext| {
                    ext.eq_ignore_ascii_case("heic") || ext.eq_ignore_ascii_case("heif")
                });
            if !is_heic {
                continue;
            }

            let (tx, rx) = std::sync::mpsc::channel();
            self.pending_preloads.push(rx);
            std::thread::spawn(move || {
                let cached = std::fs::read(&target_path)
                    .ok()
                    .and_then(|bytes| lumia_core::decode_heic_to_png(&bytes).ok())
                    .map(|img| (target_path, img));
                let _ = tx.send(cached);
            });
        }
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
        let image_count = self.image_list.len();
        if image_count == 0 {
            return;
        }
        let Some(current_idx) = self.current_image_index() else {
            return;
        };
        let new_idx = (current_idx as i32 + step).rem_euclid(image_count as i32) as usize;
        if new_idx != current_idx {
            let path = self.image_list[new_idx].clone();
            self.load_image(path, Some(window), cx);
        }
    }

    pub(crate) fn next_image(
        &mut self,
        _: &NextImage,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
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
        let (image_width, image_height) = self.display_image_dimensions()?;
        let viewport_size = window.viewport_size();
        let available_width = f32::from(viewport_size.width).max(1.0);
        let available_height = f32::from(viewport_size.height).max(1.0);
        let image_width = image_width as f32;
        let image_height = image_height as f32;
        let fit_scale = (available_width / image_width)
            .min(available_height / image_height)
            .min(1.0);
        let scale = fit_scale * self.viewport.zoom;

        Some((image_width * scale, image_height * scale))
    }

    pub(crate) fn display_image_dimensions(&self) -> Option<(u32, u32)> {
        if let Some(rotated) = self.rotated_image.as_ref() {
            return Some((rotated.width, rotated.height));
        }

        let metadata = self.current_image.as_ref()?.metadata.as_ref()?;
        if self.rotation_quarter_turns % 2 == 1 {
            Some((metadata.height, metadata.width))
        } else {
            Some((metadata.width, metadata.height))
        }
    }
    const STATUS_BAR_HOVER_ZONE_HEIGHT: f32 = STATUS_BAR_HEIGHT + 24.0;

    pub(crate) fn handle_root_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let viewport_size = window.viewport_size();
        let viewport_width = f32::from(viewport_size.width);
        let viewport_height = f32::from(viewport_size.height);
        let x = f32::from(event.position.x);
        let y = f32::from(event.position.y);
        let bottom_distance = viewport_height - y;
        let in_status_bar_zone = bottom_distance <= Self::STATUS_BAR_HOVER_ZONE_HEIGHT;
        let in_zoom_menu_zone = self.show_zoom_menu && {
            let menu_left = viewport_width - ZOOM_MENU_RIGHT - ZOOM_MENU_WIDTH;
            let menu_right = viewport_width - ZOOM_MENU_RIGHT;
            let menu_top =
                viewport_height - STATUS_BAR_HEIGHT - ZOOM_MENU_BOTTOM_GAP - ZOOM_MENU_HEIGHT;
            let menu_bottom = viewport_height - STATUS_BAR_HEIGHT;

            x >= menu_left - ZOOM_MENU_HOVER_MARGIN
                && x <= menu_right + ZOOM_MENU_HOVER_MARGIN
                && y >= menu_top - ZOOM_MENU_HOVER_MARGIN
                && y <= menu_bottom + ZOOM_MENU_HOVER_MARGIN
        };
        let should_show = in_status_bar_zone || in_zoom_menu_zone;
        if self.show_status_bar != should_show {
            self.show_status_bar = should_show;
            if !should_show {
                self.show_zoom_menu = false;
            }
            cx.notify();
        }
    }
}

impl Focusable for LumiaApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
