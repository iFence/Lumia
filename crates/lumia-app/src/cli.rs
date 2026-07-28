use std::{ffi::OsString, path::PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum CliCommand {
    /// Open the app normally (no arguments).
    Normal,
    /// Open a specific image file (first positional argument).
    OpenFile(PathBuf),
    /// Register Lumia in the OS context menu.
    RegisterContextMenu,
    /// Remove Lumia from the OS context menu.
    UnregisterContextMenu,
    /// Verify a signed plugin package without installing it.
    VerifyPluginPackage(PathBuf),
    /// Refresh only legacy Program Files file-association paths after migration.
    #[cfg(target_os = "windows")]
    RepairFileAssociations,
}

/// Parse command-line arguments into a [`CliCommand`].
///
/// The first positional argument (not starting with `-`) is treated as an image
/// file to open — this matches what all three OSes pass when launching via
/// "Open With" or double-click on an associated file.
pub(crate) fn parse() -> CliCommand {
    parse_args(std::env::args_os().skip(1))
}

fn parse_args(mut args: impl Iterator<Item = OsString>) -> CliCommand {
    match args.next().as_deref() {
        None => CliCommand::Normal,
        Some(arg) if arg == "--register-context-menu" => CliCommand::RegisterContextMenu,
        Some(arg) if arg == "--unregister-context-menu" => CliCommand::UnregisterContextMenu,
        Some(arg) if arg == "--verify-plugin-package" => {
            CliCommand::VerifyPluginPackage(args.next().map(PathBuf::from).unwrap_or_default())
        }
        #[cfg(target_os = "windows")]
        Some(arg) if arg == "--repair-file-associations" => CliCommand::RepairFileAssociations,
        Some(arg) => {
            // Treat the first non-flag argument as a file path to open.
            CliCommand::OpenFile(PathBuf::from(arg))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command(args: &[&str]) -> CliCommand {
        parse_args(args.iter().map(OsString::from))
    }

    #[test]
    fn parses_normal_and_file_open_commands() {
        assert_eq!(command(&[]), CliCommand::Normal);
        assert_eq!(
            command(&[r"C:\Pictures\sample.png"]),
            CliCommand::OpenFile(r"C:\Pictures\sample.png".into())
        );
    }

    #[test]
    fn parses_plugin_package_verification_command() {
        assert_eq!(
            command(&["--verify-plugin-package", "annotation.lumiaplugin"]),
            CliCommand::VerifyPluginPackage("annotation.lumiaplugin".into())
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn parses_installer_association_repair_command() {
        assert_eq!(
            command(&["--repair-file-associations"]),
            CliCommand::RepairFileAssociations
        );
    }
}
