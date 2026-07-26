use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FileAssociationFeedback {
    Applied,
    Removed,
    NeedsSystemConfirmation,
    ManualRestore(Vec<String>),
    Error(String),
    SettingsLaunchError(String),
}

#[derive(Debug, Default)]
pub(crate) struct FileAssociationUiState {
    pub(crate) initialized: bool,
    pub(crate) is_busy: bool,
    pub(crate) applied_extensions: BTreeSet<String>,
    pub(crate) selected_extensions: BTreeSet<String>,
    pub(crate) effective_extensions: BTreeSet<String>,
    pub(crate) feedback: Option<FileAssociationFeedback>,
}

impl FileAssociationUiState {
    pub(crate) fn is_dirty(&self) -> bool {
        self.selected_extensions != self.applied_extensions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirty_state_tracks_pending_selection() {
        let mut state = FileAssociationUiState::default();
        assert!(!state.is_dirty());

        state.selected_extensions.insert("png".into());
        assert!(state.is_dirty());

        state.applied_extensions.insert("png".into());
        assert!(!state.is_dirty());
    }
}
