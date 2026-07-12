use std::collections::BTreeSet;

use gpui::Context;
use lumia_core::SUPPORTED_IMAGE_EXTENSIONS;

use crate::app::LumiaApp;
use crate::file_association_state::FileAssociationFeedback;
use crate::shell;

impl LumiaApp {
    pub(crate) fn initialize_file_associations(&mut self, cx: &mut Context<Self>) {
        let state = &mut self.ui.file_associations;
        state.initialized = true;
        state.feedback = None;
        match shell::query_file_associations() {
            Ok(snapshot) => {
                state.registered_extensions = snapshot.registered_extensions;
                state.selected_extensions = if snapshot.configured {
                    snapshot.selected_extensions
                } else if state.registered_extensions.is_empty() {
                    all_extensions()
                } else {
                    state.registered_extensions.clone()
                };
            }
            Err(error) => {
                state.selected_extensions = all_extensions();
                state.feedback = Some(FileAssociationFeedback::Error(error.to_string()));
            }
        }
        cx.notify();
    }

    pub(crate) fn set_file_association_group(
        &mut self,
        extensions: &'static [&'static str],
        selected: bool,
        cx: &mut Context<Self>,
    ) {
        for extension in extensions {
            if selected {
                self.ui
                    .file_associations
                    .selected_extensions
                    .insert((*extension).to_string());
            } else {
                self.ui
                    .file_associations
                    .selected_extensions
                    .remove(*extension);
            }
        }
        self.ui.file_associations.feedback = None;
        cx.notify();
    }

    pub(crate) fn select_all_file_associations(&mut self, cx: &mut Context<Self>) {
        self.ui.file_associations.selected_extensions = all_extensions();
        self.ui.file_associations.feedback = None;
        cx.notify();
    }

    pub(crate) fn clear_file_associations(&mut self, cx: &mut Context<Self>) {
        self.ui.file_associations.selected_extensions.clear();
        self.ui.file_associations.feedback = None;
        cx.notify();
    }

    pub(crate) fn apply_selected_file_associations(&mut self, cx: &mut Context<Self>) {
        let selected = self.ui.file_associations.selected_extensions.clone();
        match shell::apply_file_associations(&selected) {
            Ok(()) => {
                self.ui.file_associations.registered_extensions = selected.clone();
                self.ui.file_associations.feedback = Some(if selected.is_empty() {
                    FileAssociationFeedback::Removed
                } else {
                    FileAssociationFeedback::Applied
                });
                if !selected.is_empty() {
                    if let Err(error) = shell::open_default_apps_settings() {
                        self.ui.file_associations.feedback = Some(
                            FileAssociationFeedback::SettingsLaunchError(error.to_string()),
                        );
                    }
                }
            }
            Err(error) => {
                self.refresh_registered_file_associations();
                self.ui.file_associations.feedback =
                    Some(FileAssociationFeedback::Error(error.to_string()));
            }
        }
        cx.notify();
    }

    pub(crate) fn retry_default_apps_settings(&mut self, cx: &mut Context<Self>) {
        self.ui.file_associations.feedback = match shell::open_default_apps_settings() {
            Ok(()) => Some(FileAssociationFeedback::Applied),
            Err(error) => Some(FileAssociationFeedback::SettingsLaunchError(
                error.to_string(),
            )),
        };
        cx.notify();
    }

    fn refresh_registered_file_associations(&mut self) {
        if let Ok(snapshot) = shell::query_file_associations() {
            self.ui.file_associations.registered_extensions = snapshot.registered_extensions;
        }
    }
}

fn all_extensions() -> BTreeSet<String> {
    SUPPORTED_IMAGE_EXTENSIONS
        .iter()
        .map(|extension| (*extension).to_string())
        .collect()
}
