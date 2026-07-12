use std::path::PathBuf;

use gpui::{App, Context, FocusHandle, Focusable, Subscription, WeakEntity, Window};
use lumia_core::{AppSettings, FolderNavigation, ViewerSession};

use crate::large_image::LargeImageSession;
use crate::load_state::ImageLoadState;
use crate::load_state::PreparedImage;
use crate::persistence::load_settings;
use crate::ui_state::UiState;
use crate::APP_TITLE;

pub(crate) struct LumiaApp {
    pub(crate) self_handle: WeakEntity<LumiaApp>,
    pub(crate) focus_handle: FocusHandle,
    pub(crate) viewer: ViewerSession,
    pub(crate) navigation: FolderNavigation,
    pub(crate) loads: ImageLoadState,
    pub(crate) large_image: LargeImageSession<PreparedImage>,
    pub(crate) ui: UiState,
    pub(crate) settings: AppSettings,
    pub(crate) appearance_subscription: Option<Subscription>,
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

        let mut app = Self {
            self_handle: WeakEntity::new_invalid(),
            focus_handle,
            viewer: ViewerSession::default(),
            navigation: FolderNavigation::default(),
            loads: ImageLoadState::default(),
            large_image: LargeImageSession::default(),
            ui: UiState::default(),
            settings: load_settings(),
            appearance_subscription: None,
            window_title: APP_TITLE.to_string(),
        };
        app.appearance_subscription = Some(cx.observe_window_appearance(window, |_, _, cx| {
            cx.notify();
        }));
        app.rebuild_keybindings(cx);
        window.toggle_fullscreen();
        app.ui.is_fullscreen = true;

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
        self.ui.show_settings_panel
    }
}

impl Focusable for LumiaApp {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
