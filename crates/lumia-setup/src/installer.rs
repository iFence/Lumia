use std::{path::Path, process::Command};

use anyhow::{bail, Context as _};
use winreg::{enums::HKEY_CURRENT_USER, RegKey};

const INSTALLER_KEY: &str = "Software\\Lumia\\Installer";

pub(crate) fn install(msi_path: &Path) -> anyhow::Result<()> {
    let status = Command::new("msiexec.exe")
        .args(["/i", &msi_path.to_string_lossy(), "/norestart"])
        .status()
        .context("start Windows Installer")?;
    accept_installer_status(status.code(), "install Lumia")
}

pub(crate) fn repair_file_associations() -> anyhow::Result<()> {
    let install_dir = installed_directory()?;
    let executable = install_dir.join("lumia-app.exe");
    if !executable.is_file() {
        bail!("the installed Lumia executable was not found");
    }
    let status = Command::new(&executable)
        .arg("--repair-file-associations")
        .status()
        .context("start Lumia's file-association repair")?;
    if status.success() {
        Ok(())
    } else {
        bail!(
            "Lumia's file-association repair returned exit code {}",
            status.code().unwrap_or(-1)
        )
    }
}

fn installed_directory() -> anyhow::Result<std::path::PathBuf> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey(INSTALLER_KEY) {
        if let Ok(value) = key.get_value::<String, _>("LastInstallDir") {
            return Ok(value.into());
        }
    }
    let local_app_data = std::env::var_os("LOCALAPPDATA").context("LOCALAPPDATA is not set")?;
    Ok(std::path::PathBuf::from(local_app_data)
        .join("Programs")
        .join("Lumia"))
}

pub(crate) fn accept_installer_status(code: Option<i32>, action: &str) -> anyhow::Result<()> {
    match code {
        Some(0 | 3010) => Ok(()),
        Some(1602) => bail!("{action} was cancelled"),
        Some(code) => bail!("Windows Installer could not {action} (exit code {code})"),
        None => bail!("Windows Installer terminated before it could {action}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installer_success_and_reboot_codes_are_accepted() {
        assert!(accept_installer_status(Some(0), "test").is_ok());
        assert!(accept_installer_status(Some(3010), "test").is_ok());
        assert!(accept_installer_status(Some(1603), "test").is_err());
    }
}
