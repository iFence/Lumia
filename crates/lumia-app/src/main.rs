use gpui::{
    actions, div, img, prelude::FluentBuilder, px, rgb, size, App, AppContext, Application, Bounds,
    ClickEvent, Context, ExternalPaths, FocusHandle, Focusable, InteractiveElement, IntoElement,
    KeyBinding, MouseButton, MouseDownEvent, MouseMoveEvent, ObjectFit, ParentElement, Render,
    ScrollDelta, ScrollWheelEvent, StatefulInteractiveElement, Styled, StyledImage, Subscription,
    Window, WindowAppearance, WindowBounds, WindowOptions,
};
use lumia_core::{
    supported_image_extensions, AppSettings, ImageDocument, ImageLoadError, ImageSource, Language,
    SettingsGroup, ThemeMode, ViewportState,
};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

const TOOLBAR_HEIGHT: f32 = 48.0;
const APP_TITLE: &str = "Lumia";

actions!(
    lumia,
    [
        OpenFile,
        ZoomIn,
        ZoomOut,
        ZoomFit,
        ToggleFullscreen,
        ExitFullscreen,
        ToggleImageInfo,
        Quit
    ]
);

struct LumiaApp {
    focus_handle: FocusHandle,
    viewport: ViewportState,
    current_image: Option<ImageDocument>,
    error_message: Option<String>,
    pending_drop_paths: Vec<PathBuf>,
    is_panning: bool,
    is_fullscreen: bool,
    show_image_info: bool,
    context_menu_position: Option<gpui::Point<gpui::Pixels>>,
    last_mouse_position: Option<gpui::Point<gpui::Pixels>>,
    show_settings_panel: bool,
    active_settings_group: SettingsGroup,
    settings: AppSettings,
    appearance_subscription: Option<Subscription>,
}

impl LumiaApp {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle);
        let settings = load_settings();

        let mut app = Self {
            focus_handle,
            viewport: ViewportState::default(),
            current_image: None,
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
        };
        app.appearance_subscription = Some(cx.observe_window_appearance(window, |_, _, cx| {
            cx.notify();
        }));
        app
    }

    fn open_file(&mut self, _: &OpenFile, window: &mut Window, cx: &mut Context<Self>) {
        self.open_file_dialog(cx, Some(window));
    }

    fn open_file_dialog(&mut self, cx: &mut Context<Self>, window: Option<&mut Window>) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Images", supported_image_extensions())
            .pick_file()
        {
            self.load_image(path, window);
            cx.notify();
        }
    }

    fn zoom_in(&mut self, _: &ZoomIn, _: &mut Window, cx: &mut Context<Self>) {
        self.viewport.zoom_in();
        cx.notify();
    }

    fn zoom_out(&mut self, _: &ZoomOut, _: &mut Window, cx: &mut Context<Self>) {
        self.viewport.zoom_out();
        cx.notify();
    }

    fn zoom_fit(&mut self, _: &ZoomFit, _: &mut Window, cx: &mut Context<Self>) {
        self.reset_fit(cx);
    }

    fn reset_fit(&mut self, cx: &mut Context<Self>) {
        self.viewport.reset_fit();
        cx.notify();
    }

    fn toggle_fullscreen(
        &mut self,
        _: &ToggleFullscreen,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_window_fullscreen(window, cx);
    }

    fn toggle_window_fullscreen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.toggle_fullscreen();
        self.is_fullscreen = !self.is_fullscreen;
        cx.notify();
    }

    fn exit_fullscreen(&mut self, _: &ExitFullscreen, window: &mut Window, cx: &mut Context<Self>) {
        if self.is_fullscreen || window.is_fullscreen() {
            window.toggle_fullscreen();
            self.is_fullscreen = false;
            cx.notify();
        }
    }

    fn toggle_image_info(&mut self, _: &ToggleImageInfo, _: &mut Window, cx: &mut Context<Self>) {
        self.show_image_info = !self.show_image_info;
        cx.notify();
    }

    fn open_settings_panel(&mut self, cx: &mut Context<Self>) {
        self.show_settings_panel = true;
        self.active_settings_group = SettingsGroup::General;
        self.context_menu_position = None;
        cx.notify();
    }

    fn close_settings_panel(&mut self, cx: &mut Context<Self>) {
        self.show_settings_panel = false;
        cx.notify();
    }

    fn select_settings_group(&mut self, group: SettingsGroup, cx: &mut Context<Self>) {
        self.active_settings_group = group;
        cx.notify();
    }

    fn set_language(&mut self, language: Language, cx: &mut Context<Self>) {
        self.settings.language = language;
        let _ = save_settings(&self.settings);
        cx.notify();
    }

    fn set_theme(&mut self, theme: ThemeMode, cx: &mut Context<Self>) {
        self.settings.theme = theme;
        let _ = save_settings(&self.settings);
        cx.notify();
    }

    fn load_image(&mut self, path: PathBuf, window: Option<&mut Window>) {
        match ImageDocument::load_from_path(&path) {
            Ok(document) => {
                self.current_image = Some(document);
                self.error_message = None;
                self.viewport.reset_fit();
                self.is_panning = false;
                self.context_menu_position = None;
                self.last_mouse_position = None;
                if let Some(window) = window {
                    window.set_window_title(&self.image_name());
                }
            }
            Err(error) => {
                self.error_message = Some(format_load_error(&error));
                self.is_panning = false;
                self.context_menu_position = None;
                self.last_mouse_position = None;
            }
        }
    }

    fn load_first_supported_drop(&mut self, window: &mut Window) {
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
            Some(path) => self.load_image(path, Some(window)),
            None => {
                self.error_message = Some("No supported image found in dropped files".to_string());
            }
        }
        self.pending_drop_paths.clear();
    }

    fn image_path(&self) -> Option<&Path> {
        match self.current_image.as_ref().map(|document| &document.source) {
            Some(ImageSource::LocalPath(path) | ImageSource::TemporaryPath(path)) => Some(path),
            None => None,
        }
    }

    fn image_name(&self) -> String {
        self.image_path()
            .and_then(|path| path.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("No image")
            .to_string()
    }

    fn scaled_image_size(&self, window: &Window) -> Option<(f32, f32)> {
        let metadata = self.current_image.as_ref()?.metadata.as_ref()?;
        let viewport_size = window.viewport_size();
        let available_width = f32::from(viewport_size.width).max(1.0);
        let chrome_height = if self.is_fullscreen {
            0.0
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
}

impl Focusable for LumiaApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for LumiaApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette(window);

        div()
            .id("lumia-root")
            .track_focus(&self.focus_handle)
            .key_context("Lumia")
            .relative()
            .on_action(cx.listener(Self::open_file))
            .on_action(cx.listener(Self::zoom_in))
            .on_action(cx.listener(Self::zoom_out))
            .on_action(cx.listener(Self::zoom_fit))
            .on_action(cx.listener(Self::toggle_fullscreen))
            .on_action(cx.listener(Self::exit_fullscreen))
            .on_action(cx.listener(Self::toggle_image_info))
            .on_action(|_: &Quit, _: &mut Window, cx: &mut App| cx.quit())
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(palette.viewer_bg))
            .text_color(rgb(palette.text))
            .children((!self.is_fullscreen).then(|| self.render_toolbar(palette, cx)))
            .child(self.render_viewer(window, palette, cx))
            .children(self.render_settings_panel(window, palette, cx))
    }
}

impl LumiaApp {
    fn render_toolbar(&self, palette: Palette, cx: &mut Context<Self>) -> impl IntoElement {
        let language = self.settings.language;

        div()
            .id("toolbar")
            .h(px(TOOLBAR_HEIGHT))
            .w_full()
            .flex()
            .items_center()
            .gap_2()
            .px_4()
            .border_b_1()
            .border_color(rgb(palette.border))
            .bg(rgb(palette.toolbar_bg))
            .child(toolbar_button(
                "open-button",
                tr(language, TextKey::Open),
                palette,
                cx,
                |this, _, window, cx| {
                    this.open_file_dialog(cx, Some(window));
                },
            ))
            .child(toolbar_button(
                "fit-button",
                tr(language, TextKey::Fit),
                palette,
                cx,
                |this, _, _, cx| {
                    this.reset_fit(cx);
                },
            ))
            .child(toolbar_button(
                "fullscreen-button",
                tr(language, TextKey::Full),
                palette,
                cx,
                |this, _, window, cx| {
                    this.toggle_window_fullscreen(window, cx);
                },
            ))
            .child(
                div()
                    .px_2()
                    .text_sm()
                    .text_color(rgb(palette.muted_text))
                    .child(format!("{:.0}%", self.viewport.zoom * 100.0)),
            )
            .child(div().flex_1())
    }

    fn render_viewer(
        &self,
        window: &Window,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let viewer = div()
            .id("viewer")
            .flex_1()
            .overflow_hidden()
            .flex()
            .items_center()
            .justify_center()
            .relative()
            .bg(rgb(palette.viewer_bg))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                this.pending_drop_paths = paths.paths().to_vec();
                this.load_first_supported_drop(window);
                cx.notify();
            }))
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                let delta = match event.delta {
                    ScrollDelta::Pixels(delta) => f32::from(delta.y),
                    ScrollDelta::Lines(delta) => delta.y,
                };
                if delta > 0.0 {
                    this.viewport.zoom_out();
                } else if delta < 0.0 {
                    this.viewport.zoom_in();
                }
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    if this.context_menu_position.take().is_some() {
                        cx.notify();
                        return;
                    }
                    if this.current_image.is_some() {
                        this.is_panning = true;
                        this.last_mouse_position = Some(event.position);
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.is_panning = false;
                    this.last_mouse_position = None;
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|this, event: &MouseDownEvent, _, cx| {
                    let chrome_height = if this.is_fullscreen {
                        0.0
                    } else {
                        TOOLBAR_HEIGHT
                    };
                    this.context_menu_position = Some(gpui::point(
                        event.position.x,
                        px((f32::from(event.position.y) - chrome_height).max(0.0)),
                    ));
                    this.is_panning = false;
                    this.last_mouse_position = None;
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.is_panning = false;
                    this.last_mouse_position = None;
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                if this.is_panning && event.dragging() {
                    if let Some(last_position) = this.last_mouse_position {
                        this.viewport.pan_by(
                            f32::from(event.position.x - last_position.x),
                            f32::from(event.position.y - last_position.y),
                        );
                    }
                    this.last_mouse_position = Some(event.position);
                    cx.notify();
                }
            }));

        if let Some(message) = &self.error_message {
            viewer
                .child(status_message("error-state", message, palette.error_text))
                .children(self.render_image_info_overlay())
                .children(self.render_context_menu(palette, cx))
        } else if let Some(path) = self.image_path() {
            let image = if let Some((width, height)) = self.scaled_image_size(window) {
                img(path.to_path_buf())
                    .w(px(width))
                    .h(px(height))
                    .object_fit(ObjectFit::Contain)
                    .into_any_element()
            } else {
                img(path.to_path_buf())
                    .max_w_full()
                    .max_h_full()
                    .object_fit(ObjectFit::Contain)
                    .into_any_element()
            };

            viewer
                .child(
                    div()
                        .ml(px(self.viewport.pan_x))
                        .mt(px(self.viewport.pan_y))
                        .child(image),
                )
                .children(self.render_image_info_overlay())
                .children(self.render_context_menu(palette, cx))
        } else {
            viewer
                .child(status_message(
                    "empty-state",
                    tr(self.settings.language, TextKey::EmptyState),
                    palette.muted_text,
                ))
                .children(self.render_image_info_overlay())
                .children(self.render_context_menu(palette, cx))
        }
    }

    fn render_context_menu(
        &self,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        let language = self.settings.language;

        self.context_menu_position.map(|position| {
            div()
                .id("viewer-context-menu")
                .absolute()
                .left(position.x)
                .top(position.y)
                .w(px(156.0))
                .py_1()
                .rounded_md()
                .bg(rgb(palette.panel_bg))
                .border_1()
                .border_color(rgb(palette.border))
                .shadow_lg()
                .text_color(rgb(palette.text))
                .text_sm()
                .child(context_menu_item(
                    "settings-menu-item",
                    tr(language, TextKey::Settings),
                    palette,
                    cx,
                    |this, _, _, cx| {
                        this.open_settings_panel(cx);
                    },
                ))
                .child(context_menu_item(
                    "about-menu-item",
                    tr(language, TextKey::About),
                    palette,
                    cx,
                    |this, _, _, cx| {
                        this.context_menu_position = None;
                        cx.notify();
                    },
                ))
                .child(context_menu_item(
                    "quit-menu-item",
                    tr(language, TextKey::Quit),
                    palette,
                    cx,
                    |this, _, _, cx| {
                        this.context_menu_position = None;
                        cx.quit();
                    },
                ))
        })
    }

    fn render_settings_panel(
        &self,
        window: &Window,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        self.show_settings_panel.then(|| {
            let language = self.settings.language;
            let viewport_size = window.viewport_size();
            let panel_width = (f32::from(viewport_size.width) - 48.0)
                .max(320.0)
                .min(780.0);
            let panel_height = (f32::from(viewport_size.height) - 48.0)
                .max(360.0)
                .min(520.0);

            div()
                .id("settings-overlay")
                .absolute()
                .left(px(0.0))
                .top(px(0.0))
                .right(px(0.0))
                .bottom(px(0.0))
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::black().opacity(0.48))
                .child(
                    div()
                        .id("settings-panel")
                        .w(px(panel_width))
                        .h(px(panel_height))
                        .overflow_hidden()
                        .rounded_md()
                        .border_1()
                        .border_color(rgb(palette.border))
                        .bg(rgb(palette.panel_bg))
                        .shadow_lg()
                        .text_color(rgb(palette.text))
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .id("settings-header")
                                .h(px(56.0))
                                .flex()
                                .items_center()
                                .gap_2()
                                .px_4()
                                .border_b_1()
                                .border_color(rgb(palette.border))
                                .child(
                                    div()
                                        .flex_1()
                                        .flex()
                                        .flex_col()
                                        .child(
                                            div()
                                                .text_sm()
                                                .child(tr(language, TextKey::SettingsTitle)),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(palette.muted_text))
                                                .child(tr(language, TextKey::SettingsDescription)),
                                        ),
                                )
                                .child(toolbar_button(
                                    "settings-close-button",
                                    tr(language, TextKey::Close),
                                    palette,
                                    cx,
                                    |this, _, _, cx| {
                                        this.close_settings_panel(cx);
                                    },
                                )),
                        )
                        .child(
                            div()
                                .id("settings-body")
                                .flex_1()
                                .flex()
                                .child(self.render_settings_sidebar(palette, cx))
                                .child(self.render_settings_content(window, palette, cx)),
                        ),
                )
        })
    }

    fn render_settings_sidebar(
        &self,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let language = self.settings.language;

        div()
            .id("settings-sidebar")
            .w(px(188.0))
            .h_full()
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .border_r_1()
            .border_color(rgb(palette.border))
            .bg(rgb(palette.sidebar_bg))
            .child(settings_group_button(
                "settings-group-general",
                tr(language, TextKey::General),
                self.active_settings_group == SettingsGroup::General,
                palette,
                cx,
                |this, _, _, cx| {
                    this.select_settings_group(SettingsGroup::General, cx);
                },
            ))
            .child(settings_group_button(
                "settings-group-shortcuts",
                tr(language, TextKey::Shortcuts),
                self.active_settings_group == SettingsGroup::Shortcuts,
                palette,
                cx,
                |this, _, _, cx| {
                    this.select_settings_group(SettingsGroup::Shortcuts, cx);
                },
            ))
    }

    fn render_settings_content(
        &self,
        window: &Window,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        match self.active_settings_group {
            SettingsGroup::General => self
                .render_general_settings(window, palette, cx)
                .into_any_element(),
            SettingsGroup::Shortcuts => self.render_shortcuts_settings(palette).into_any_element(),
        }
    }

    fn render_general_settings(
        &self,
        window: &Window,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let language = self.settings.language;
        let resolved_theme = if theme_resolves_to_dark(self.settings.theme, window.appearance()) {
            tr(language, TextKey::Dark)
        } else {
            tr(language, TextKey::Light)
        };

        div()
            .id("settings-general")
            .flex_1()
            .flex()
            .flex_col()
            .gap_5()
            .p_5()
            .child(settings_section_title(
                tr(language, TextKey::General),
                tr(language, TextKey::GeneralDescription),
                palette,
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .child(settings_label(
                        tr(language, TextKey::Language),
                        tr(language, TextKey::LanguageDescription),
                        palette,
                    ))
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(settings_option_button(
                                "language-english",
                                tr(language, TextKey::English),
                                self.settings.language == Language::English,
                                palette,
                                cx,
                                |this, _, _, cx| {
                                    this.set_language(Language::English, cx);
                                },
                            ))
                            .child(settings_option_button(
                                "language-chinese",
                                tr(language, TextKey::Chinese),
                                self.settings.language == Language::Chinese,
                                palette,
                                cx,
                                |this, _, _, cx| {
                                    this.set_language(Language::Chinese, cx);
                                },
                            )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .child(settings_label(
                        tr(language, TextKey::Theme),
                        tr(language, TextKey::ThemeDescription),
                        palette,
                    ))
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(settings_option_button(
                                "theme-light",
                                tr(language, TextKey::Light),
                                self.settings.theme == ThemeMode::Light,
                                palette,
                                cx,
                                |this, _, _, cx| {
                                    this.set_theme(ThemeMode::Light, cx);
                                },
                            ))
                            .child(settings_option_button(
                                "theme-dark",
                                tr(language, TextKey::Dark),
                                self.settings.theme == ThemeMode::Dark,
                                palette,
                                cx,
                                |this, _, _, cx| {
                                    this.set_theme(ThemeMode::Dark, cx);
                                },
                            ))
                            .child(settings_option_button(
                                "theme-system",
                                tr(language, TextKey::FollowSystem),
                                self.settings.theme == ThemeMode::FollowSystem,
                                palette,
                                cx,
                                |this, _, _, cx| {
                                    this.set_theme(ThemeMode::FollowSystem, cx);
                                },
                            )),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(palette.muted_text))
                    .child(format!(
                        "{}: {resolved_theme}",
                        tr(language, TextKey::ResolvedTheme)
                    )),
            )
    }

    fn render_shortcuts_settings(&self, palette: Palette) -> impl IntoElement {
        let language = self.settings.language;
        let shortcuts = [
            (TextKey::ShortcutOpenFile, "Ctrl+O / Cmd+O"),
            (TextKey::ShortcutZoomIn, "Ctrl++ / Cmd++"),
            (TextKey::ShortcutZoomOut, "Ctrl+- / Cmd+-"),
            (TextKey::ShortcutZoomFit, "Ctrl+0 / Cmd+0"),
            (TextKey::ShortcutFullscreen, "F11 / Ctrl+Enter / Cmd+Enter"),
            (TextKey::ShortcutImageInfo, "Tab"),
            (TextKey::ShortcutQuit, "Ctrl+Q / Cmd+Q"),
        ];

        div()
            .id("settings-shortcuts")
            .flex_1()
            .flex()
            .flex_col()
            .gap_4()
            .p_5()
            .child(settings_section_title(
                tr(language, TextKey::Shortcuts),
                tr(language, TextKey::ShortcutsDescription),
                palette,
            ))
            .children(shortcuts.into_iter().map(|(label, binding)| {
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .px_3()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(palette.subtle_bg))
                    .child(div().text_sm().child(tr(language, label)))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(palette.muted_text))
                            .child(binding),
                    )
            }))
    }

    fn render_image_info_overlay(&self) -> Option<impl IntoElement> {
        (self.show_image_info && self.image_path().is_some()).then(|| {
            div()
                .id("image-info-overlay")
                .absolute()
                .top_4()
                .left_4()
                .max_w(px(420.0))
                .px_3()
                .py_2()
                .rounded_md()
                .bg(gpui::black().opacity(0.72))
                .text_color(rgb(0xf2f2f2))
                .text_xs()
                .shadow_md()
                .children(
                    self.image_info_lines()
                        .into_iter()
                        .map(|line| div().child(line)),
                )
        })
    }

    fn image_info_lines(&self) -> Vec<String> {
        let Some(path) = self.image_path() else {
            return Vec::new();
        };

        let mut lines = Vec::new();
        lines.push(format!("Name: {}", self.image_name()));

        if let Some(metadata) = self
            .current_image
            .as_ref()
            .and_then(|image| image.metadata.as_ref())
        {
            lines.push(format!(
                "Dimensions: {} x {}",
                metadata.width, metadata.height
            ));
            if let Some(format_name) = metadata.format_name.as_deref() {
                lines.push(format!("Format: {format_name}"));
            }
        } else {
            lines.push("Dimensions: unknown".to_string());
        }

        if let Ok(file_metadata) = fs::metadata(path) {
            lines.push(format!(
                "File size: {}",
                format_file_size(file_metadata.len())
            ));
            if let Ok(modified) = file_metadata.modified() {
                lines.push(format!("Modified: {}", format_modified_time(modified)));
            }
        }

        lines.push(format!("Zoom: {:.0}%", self.viewport.zoom * 100.0));
        lines.push(format!("Path: {}", path.display()));
        lines
    }
}

fn toolbar_button(
    id: &'static str,
    label: &'static str,
    palette: Palette,
    cx: &mut Context<LumiaApp>,
    on_click: impl Fn(&mut LumiaApp, &ClickEvent, &mut Window, &mut Context<LumiaApp>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_md()
        .bg(rgb(palette.button_bg))
        .text_color(rgb(palette.text))
        .text_sm()
        .cursor_pointer()
        .hover(move |style| style.bg(rgb(palette.button_hover)))
        .on_click(cx.listener(on_click))
        .child(label)
}

fn context_menu_item(
    id: &'static str,
    label: &'static str,
    palette: Palette,
    cx: &mut Context<LumiaApp>,
    on_click: impl Fn(&mut LumiaApp, &ClickEvent, &mut Window, &mut Context<LumiaApp>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .w_full()
        .px_3()
        .py_1()
        .cursor_pointer()
        .hover(move |style| style.bg(rgb(palette.button_hover)))
        .on_click(cx.listener(on_click))
        .child(label)
}

fn settings_group_button(
    id: &'static str,
    label: &'static str,
    active: bool,
    palette: Palette,
    cx: &mut Context<LumiaApp>,
    on_click: impl Fn(&mut LumiaApp, &ClickEvent, &mut Window, &mut Context<LumiaApp>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .w_full()
        .px_3()
        .py_2()
        .rounded_md()
        .text_sm()
        .cursor_pointer()
        .when(active, move |style| {
            style
                .bg(rgb(palette.selection_bg))
                .text_color(rgb(palette.text))
        })
        .when(!active, move |style| {
            style
                .bg(rgb(palette.sidebar_bg))
                .text_color(rgb(palette.muted_text))
        })
        .hover(move |style| style.bg(rgb(palette.button_hover)))
        .on_click(cx.listener(on_click))
        .child(label)
}

fn settings_option_button(
    id: &'static str,
    label: &'static str,
    active: bool,
    palette: Palette,
    cx: &mut Context<LumiaApp>,
    on_click: impl Fn(&mut LumiaApp, &ClickEvent, &mut Window, &mut Context<LumiaApp>) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(rgb(if active {
            palette.selection_bg
        } else {
            palette.border
        }))
        .text_sm()
        .cursor_pointer()
        .when(active, move |style| {
            style
                .bg(rgb(palette.selection_bg))
                .text_color(rgb(palette.text))
        })
        .when(!active, move |style| {
            style
                .bg(rgb(palette.panel_bg))
                .text_color(rgb(palette.muted_text))
        })
        .hover(move |style| style.bg(rgb(palette.button_hover)))
        .on_click(cx.listener(on_click))
        .child(label)
}

fn settings_section_title(
    title: &'static str,
    description: &'static str,
    palette: Palette,
) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_sm().child(title))
        .child(
            div()
                .text_xs()
                .text_color(rgb(palette.muted_text))
                .child(description),
        )
}

fn settings_label(
    title: &'static str,
    description: &'static str,
    palette: Palette,
) -> impl IntoElement {
    div()
        .flex_1()
        .flex()
        .flex_col()
        .gap_1()
        .child(div().text_sm().child(title))
        .child(
            div()
                .text_xs()
                .text_color(rgb(palette.muted_text))
                .child(description),
        )
}

#[derive(Clone, Copy)]
struct Palette {
    viewer_bg: u32,
    toolbar_bg: u32,
    panel_bg: u32,
    sidebar_bg: u32,
    subtle_bg: u32,
    button_bg: u32,
    button_hover: u32,
    selection_bg: u32,
    border: u32,
    text: u32,
    muted_text: u32,
    error_text: u32,
}

impl Palette {
    fn for_theme(theme: ThemeMode, appearance: WindowAppearance) -> Self {
        if theme_resolves_to_dark(theme, appearance) {
            Self::dark()
        } else {
            Self::light()
        }
    }

    fn dark() -> Self {
        Self {
            viewer_bg: 0x202020,
            toolbar_bg: 0x181818,
            panel_bg: 0x252525,
            sidebar_bg: 0x202020,
            subtle_bg: 0x2c2c2c,
            button_bg: 0x2f2f2f,
            button_hover: 0x3a3a3a,
            selection_bg: 0x3d4a5c,
            border: 0x3c3c3c,
            text: 0xf2f2f2,
            muted_text: 0xbdbdbd,
            error_text: 0xffb3b3,
        }
    }

    fn light() -> Self {
        Self {
            viewer_bg: 0xf4f4f4,
            toolbar_bg: 0xffffff,
            panel_bg: 0xffffff,
            sidebar_bg: 0xf2f2f2,
            subtle_bg: 0xf5f5f5,
            button_bg: 0xeeeeee,
            button_hover: 0xe2e2e2,
            selection_bg: 0xdce8f8,
            border: 0xd0d0d0,
            text: 0x202020,
            muted_text: 0x5f6368,
            error_text: 0x9b1c1c,
        }
    }
}

fn theme_resolves_to_dark(theme: ThemeMode, appearance: WindowAppearance) -> bool {
    match theme {
        ThemeMode::Light => false,
        ThemeMode::Dark => true,
        ThemeMode::FollowSystem => matches!(
            appearance,
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ),
    }
}

impl LumiaApp {
    fn palette(&self, window: &Window) -> Palette {
        Palette::for_theme(self.settings.theme, window.appearance())
    }
}

#[derive(Clone, Copy)]
enum TextKey {
    Open,
    Fit,
    Full,
    Settings,
    About,
    Quit,
    EmptyState,
    Close,
    SettingsTitle,
    SettingsDescription,
    General,
    GeneralDescription,
    Shortcuts,
    ShortcutsDescription,
    Language,
    LanguageDescription,
    Theme,
    ThemeDescription,
    English,
    Chinese,
    Light,
    Dark,
    FollowSystem,
    ResolvedTheme,
    ShortcutOpenFile,
    ShortcutZoomIn,
    ShortcutZoomOut,
    ShortcutZoomFit,
    ShortcutFullscreen,
    ShortcutImageInfo,
    ShortcutQuit,
}

fn tr(language: Language, key: TextKey) -> &'static str {
    match language {
        Language::English => match key {
            TextKey::Open => "Open",
            TextKey::Fit => "Fit",
            TextKey::Full => "Full",
            TextKey::Settings => "Settings",
            TextKey::About => "About",
            TextKey::Quit => "Quit",
            TextKey::EmptyState => "Drop an image here, or open one with Ctrl+O",
            TextKey::Close => "Close",
            TextKey::SettingsTitle => "Settings",
            TextKey::SettingsDescription => "Configure Lumia preferences",
            TextKey::General => "General",
            TextKey::GeneralDescription => "Language and appearance preferences",
            TextKey::Shortcuts => "Shortcuts",
            TextKey::ShortcutsDescription => "Current keyboard shortcuts",
            TextKey::Language => "Language",
            TextKey::LanguageDescription => "Choose the display language",
            TextKey::Theme => "Theme",
            TextKey::ThemeDescription => "Choose light, dark, or system appearance",
            TextKey::English => "English",
            TextKey::Chinese => "中文",
            TextKey::Light => "Light",
            TextKey::Dark => "Dark",
            TextKey::FollowSystem => "Follow System",
            TextKey::ResolvedTheme => "Resolved theme",
            TextKey::ShortcutOpenFile => "Open file",
            TextKey::ShortcutZoomIn => "Zoom in",
            TextKey::ShortcutZoomOut => "Zoom out",
            TextKey::ShortcutZoomFit => "Fit to window",
            TextKey::ShortcutFullscreen => "Toggle fullscreen",
            TextKey::ShortcutImageInfo => "Toggle image info",
            TextKey::ShortcutQuit => "Quit",
        },
        Language::Chinese => match key {
            TextKey::Open => "打开",
            TextKey::Fit => "适应",
            TextKey::Full => "全屏",
            TextKey::Settings => "设置",
            TextKey::About => "关于",
            TextKey::Quit => "退出",
            TextKey::EmptyState => "将图片拖到这里，或按 Ctrl+O 打开",
            TextKey::Close => "关闭",
            TextKey::SettingsTitle => "设置",
            TextKey::SettingsDescription => "配置 Lumia 偏好设置",
            TextKey::General => "通用",
            TextKey::GeneralDescription => "语言和外观偏好",
            TextKey::Shortcuts => "快捷键",
            TextKey::ShortcutsDescription => "当前键盘快捷键",
            TextKey::Language => "语言",
            TextKey::LanguageDescription => "选择界面显示语言",
            TextKey::Theme => "主题",
            TextKey::ThemeDescription => "选择浅色、深色或跟随系统",
            TextKey::English => "English",
            TextKey::Chinese => "中文",
            TextKey::Light => "浅色",
            TextKey::Dark => "深色",
            TextKey::FollowSystem => "跟随系统",
            TextKey::ResolvedTheme => "当前解析主题",
            TextKey::ShortcutOpenFile => "打开文件",
            TextKey::ShortcutZoomIn => "放大",
            TextKey::ShortcutZoomOut => "缩小",
            TextKey::ShortcutZoomFit => "适应窗口",
            TextKey::ShortcutFullscreen => "切换全屏",
            TextKey::ShortcutImageInfo => "切换图片信息",
            TextKey::ShortcutQuit => "退出",
        },
    }
}

fn load_settings() -> AppSettings {
    settings_path()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn save_settings(settings: &AppSettings) -> io::Result<()> {
    let Some(path) = settings_path() else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(settings).map_err(io::Error::other)?;
    fs::write(path, json)
}

fn settings_path() -> Option<PathBuf> {
    platform_config_dir().map(|dir| dir.join("settings.json"))
}

fn platform_config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .map(|path| path.join(APP_TITLE))
    }

    #[cfg(target_os = "macos")]
    {
        env::var_os("HOME").map(PathBuf::from).map(|path| {
            path.join("Library")
                .join("Application Support")
                .join(APP_TITLE)
        })
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .map(|path| path.join("lumia"))
    }
}

fn status_message(id: &'static str, message: impl Into<String>, color: u32) -> impl IntoElement {
    div()
        .id(id)
        .px_4()
        .py_3()
        .text_center()
        .text_color(rgb(color))
        .child(message.into())
}

fn format_file_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit = 0;

    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}

fn format_modified_time(modified: std::time::SystemTime) -> String {
    match modified.duration_since(UNIX_EPOCH) {
        Ok(duration) => format!("{} unix seconds", duration.as_secs()),
        Err(_) => "before unix epoch".to_string(),
    }
}

fn format_load_error(error: &ImageLoadError) -> String {
    match error {
        ImageLoadError::UnsupportedExtension(extension) => {
            format!("Unsupported image format: .{extension}")
        }
        ImageLoadError::MissingExtension(_) => "The selected file has no extension".to_string(),
        ImageLoadError::NotFound(_) => "The selected file no longer exists".to_string(),
        ImageLoadError::NotAFile(_) => "The selected path is not a file".to_string(),
        ImageLoadError::Metadata { .. } | ImageLoadError::Io { .. } => {
            "Could not read image metadata".to_string()
        }
    }
}

fn main() -> anyhow::Result<()> {
    Application::new().run(|cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("cmd-o", OpenFile, None),
            KeyBinding::new("ctrl-o", OpenFile, None),
            KeyBinding::new("cmd-plus", ZoomIn, None),
            KeyBinding::new("cmd-equals", ZoomIn, None),
            KeyBinding::new("ctrl-plus", ZoomIn, None),
            KeyBinding::new("ctrl-equals", ZoomIn, None),
            KeyBinding::new("cmd-minus", ZoomOut, None),
            KeyBinding::new("ctrl-minus", ZoomOut, None),
            KeyBinding::new("cmd-0", ZoomFit, None),
            KeyBinding::new("ctrl-0", ZoomFit, None),
            KeyBinding::new("f11", ToggleFullscreen, None),
            KeyBinding::new("cmd-enter", ToggleFullscreen, None),
            KeyBinding::new("ctrl-enter", ToggleFullscreen, None),
            KeyBinding::new("escape", ExitFullscreen, None),
            KeyBinding::new("tab", ToggleImageInfo, None),
            KeyBinding::new("cmd-q", Quit, None),
            KeyBinding::new("ctrl-q", Quit, None),
        ]);
        cx.on_action(|_: &Quit, cx| cx.quit());

        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some(APP_TITLE.into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| LumiaApp::new(window, cx)),
        )
        .expect("failed to open Lumia window");
        cx.activate(true);
    });

    Ok(())
}
