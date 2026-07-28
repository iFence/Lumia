use std::path::PathBuf;

use gpui::{App, Context, FocusHandle, Focusable, Subscription, WeakEntity, Window};
use lumia_core::{AnnotationDocument, AppSettings, FolderNavigation, SettingsGroup, ViewerSession};

use crate::editing::EditState;
use crate::large_image::LargeImageSession;
use crate::load_state::ImageLoadState;
use crate::load_state::PreparedImage;
use crate::persistence::load_settings;
use crate::plugin_state::PluginUiState;
use crate::preview_cache::{PreviewCache, PreviewPreloadState};
use crate::slideshow::SlideshowState;
use crate::ui_state::UiState;
use crate::APP_TITLE;

pub(crate) struct LumiaApp {
    pub(crate) self_handle: WeakEntity<LumiaApp>,
    pub(crate) focus_handle: FocusHandle,
    pub(crate) viewer: ViewerSession,
    pub(crate) navigation: FolderNavigation,
    pub(crate) loads: ImageLoadState,
    pub(crate) preview_cache: PreviewCache,
    pub(crate) preview_preloads: PreviewPreloadState,
    pub(crate) navigation_direction: i32,
    pub(crate) large_image: LargeImageSession<PreparedImage>,
    pub(crate) editing: EditState,
    pub(crate) slideshow: SlideshowState,
    pub(crate) plugins: PluginUiState,
    pub(crate) annotations: AnnotationDocument,
    pub(crate) ui: UiState,
    pub(crate) settings: AppSettings,
    pub(crate) appearance_subscription: Option<Subscription>,
    pub(crate) activation_subscription: Option<Subscription>,
    pub(crate) window_active: bool,
    pub(crate) window_title: String,
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
        crate::shell::apply_native_theme(settings.theme);
        let mut app = Self {
            self_handle: WeakEntity::new_invalid(),
            focus_handle,
            viewer: ViewerSession::default(),
            navigation: FolderNavigation::default(),
            loads: ImageLoadState::default(),
            preview_cache: PreviewCache::default(),
            preview_preloads: PreviewPreloadState::default(),
            navigation_direction: 1,
            large_image: LargeImageSession::default(),
            editing: EditState::default(),
            slideshow: SlideshowState::default(),
            plugins: PluginUiState::new(),
            annotations: AnnotationDocument::default(),
            ui: UiState::default(),
            settings,
            appearance_subscription: None,
            activation_subscription: None,
            window_active: window.is_window_active(),
            window_title: APP_TITLE.to_string(),
        };
        app.appearance_subscription = Some(cx.observe_window_appearance(window, |_, _, cx| {
            cx.notify();
        }));
        app.activation_subscription =
            Some(cx.observe_window_activation(window, |app, window, cx| {
                let was_active = app.window_active;
                app.window_active = window.is_window_active();
                if app.window_active
                    && !was_active
                    && app.ui.show_settings_panel
                    && app.ui.active_settings_group == SettingsGroup::FileAssociations
                {
                    app.refresh_file_associations(cx);
                }
                cx.notify();
            }));
        app.rebuild_keybindings(cx);

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

    pub(crate) fn is_viewer_blocked(&self) -> bool {
        self.ui.show_settings_panel || self.editing.mode.is_some()
    }
}

impl Focusable for LumiaApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
