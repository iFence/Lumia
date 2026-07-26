use std::collections::BTreeSet;

use gpui::Context;
use lumia_core::SUPPORTED_IMAGE_EXTENSIONS;

use crate::app::LumiaApp;
use crate::file_association_state::FileAssociationFeedback;
use crate::shell;

impl LumiaApp {
    pub(crate) fn initialize_file_associations(&mut self, cx: &mut Context<Self>) {
        if self.ui.file_associations.is_busy {
            return;
        }
        self.ui.file_associations.initialized = true;
        self.ui.file_associations.is_busy = true;
        self.ui.file_associations.feedback = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let snapshot = cx
                .background_executor()
                .spawn(async { shell::query_file_associations() })
                .await;
            let _ = this.update(cx, |this, cx| {
                let state = &mut this.ui.file_associations;
                state.is_busy = false;
                match snapshot {
                    Ok(snapshot) => {
                        let effective_extensions = snapshot.effective_extensions;
                        state.applied_extensions = if snapshot.configured {
                            snapshot.selected_extensions.clone()
                        } else {
                            effective_extensions.clone()
                        };
                        state.selected_extensions = if snapshot.configured {
                            snapshot.selected_extensions
                        } else if effective_extensions.is_empty() {
                            all_extensions()
                        } else {
                            effective_extensions.clone()
                        };
                        state.effective_extensions = effective_extensions;
                    }
                    Err(error) => {
                        if state.selected_extensions.is_empty() {
                            state.selected_extensions = all_extensions();
                        }
                        state.feedback = Some(FileAssociationFeedback::Error(error.to_string()));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn refresh_file_associations(&mut self, cx: &mut Context<Self>) {
        self.ui.file_associations.initialized = false;
        self.initialize_file_associations(cx);
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
        if self.ui.file_associations.is_busy {
            return;
        }
        let selected = self.ui.file_associations.selected_extensions.clone();
        self.ui.file_associations.is_busy = true;
        self.ui.file_associations.feedback = None;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let applied = shell::apply_file_associations(&selected)?;
                    let settings_error = if applied.system_confirmation_required {
                        shell::open_default_apps_settings().err()
                    } else {
                        None
                    };
                    Ok::<_, anyhow::Error>((applied, settings_error))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let state = &mut this.ui.file_associations;
                state.is_busy = false;
                match result {
                    Ok((applied, settings_error)) => {
                        state.applied_extensions = applied.snapshot.selected_extensions.clone();
                        state.selected_extensions = applied.snapshot.selected_extensions;
                        state.effective_extensions = applied.snapshot.effective_extensions;
                        state.feedback = if let Some(error) = settings_error {
                            Some(FileAssociationFeedback::SettingsLaunchError(
                                error.to_string(),
                            ))
                        } else if applied.system_confirmation_required {
                            Some(FileAssociationFeedback::NeedsSystemConfirmation)
                        } else if !applied.manual_restore_extensions.is_empty() {
                            Some(FileAssociationFeedback::ManualRestore(
                                applied
                                    .manual_restore_extensions
                                    .into_iter()
                                    .map(|extension| format!(".{extension}"))
                                    .collect(),
                            ))
                        } else if state.applied_extensions.is_empty() {
                            Some(FileAssociationFeedback::Removed)
                        } else {
                            Some(FileAssociationFeedback::Applied)
                        };
                    }
                    Err(error) => {
                        state.feedback = Some(FileAssociationFeedback::Error(error.to_string()));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn retry_default_apps_settings(&mut self, cx: &mut Context<Self>) {
        self.ui.file_associations.feedback = match shell::open_default_apps_settings() {
            Ok(()) => Some(FileAssociationFeedback::NeedsSystemConfirmation),
            Err(error) => Some(FileAssociationFeedback::SettingsLaunchError(
                error.to_string(),
            )),
        };
        cx.notify();
    }
}

fn all_extensions() -> BTreeSet<String> {
    SUPPORTED_IMAGE_EXTENSIONS
        .iter()
        .map(|extension| (*extension).to_string())
        .collect()
}
