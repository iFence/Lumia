use std::path::PathBuf;

pub(crate) enum CliCommand {
    /// Open the app normally (no arguments).
    Normal,
    /// Open a specific image file (first positional argument).
    OpenFile(PathBuf),
    /// Register Lumia in the OS context menu.
    RegisterContextMenu,
    /// Remove Lumia from the OS context menu.
    UnregisterContextMenu,
}

/// Parse command-line arguments into a [`CliCommand`].
///
/// The first positional argument (not starting with `-`) is treated as an image
/// file to open — this matches what all three OSes pass when launching via
/// "Open With" or double-click on an associated file.
pub(crate) fn parse() -> CliCommand {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None => CliCommand::Normal,
        Some("--register-context-menu") => CliCommand::RegisterContextMenu,
        Some("--unregister-context-menu") => CliCommand::UnregisterContextMenu,
        Some(arg) => {
            // Treat the first non-flag argument as a file path to open.
            CliCommand::OpenFile(PathBuf::from(arg))
        }
    }
}
