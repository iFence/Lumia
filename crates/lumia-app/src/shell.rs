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
