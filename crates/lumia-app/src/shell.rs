#[cfg(target_os = "windows")]
use std::collections::BTreeSet;

#[cfg(all(unix, not(target_os = "macos")))]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(all(unix, not(target_os = "macos")))]
use linux as platform;
#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(target_os = "windows")]
use windows as platform;

/// Register Lumia in the operating-system context menu.
pub(crate) fn register_context_menu() -> anyhow::Result<()> {
    platform::register(&std::env::current_exe()?)
}

/// Remove Lumia from the operating-system context menu.
pub(crate) fn unregister_context_menu() -> anyhow::Result<()> {
    platform::unregister()
}

#[cfg(target_os = "windows")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FileAssociationSnapshot {
    pub(crate) configured: bool,
    pub(crate) registered_extensions: BTreeSet<String>,
    pub(crate) selected_extensions: BTreeSet<String>,
}

#[cfg(target_os = "windows")]
pub(crate) fn query_file_associations() -> anyhow::Result<FileAssociationSnapshot> {
    platform::query(&std::env::current_exe()?)
}

#[cfg(target_os = "windows")]
pub(crate) fn apply_file_associations(
    selected_extensions: &BTreeSet<String>,
) -> anyhow::Result<()> {
    platform::apply(&std::env::current_exe()?, selected_extensions)
}

#[cfg(target_os = "windows")]
pub(crate) fn open_default_apps_settings() -> anyhow::Result<()> {
    platform::open_default_apps_settings()
}

#[cfg(target_os = "windows")]
pub(crate) fn repair_legacy_file_associations() -> anyhow::Result<()> {
    platform::repair_legacy_associations(&std::env::current_exe()?)
}
