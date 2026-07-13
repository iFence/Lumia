use std::{path::Path, process::Command};

use anyhow::{bail, Context as _};
use windows_sys::Win32::{
    Foundation::{ERROR_NO_MORE_ITEMS, ERROR_SUCCESS},
    System::ApplicationInstallationAndServicing::{
        MsiEnumRelatedProductsW, MsiGetProductInfoW, INSTALLPROPERTY_INSTALLLOCATION,
    },
};

use crate::installer::accept_installer_status;

const UPGRADE_CODE: &str = "{07618386-59AC-4B5C-B4E1-EBE4F8AA182E}";
const PRODUCT_CODE_CAPACITY: usize = 39;

pub(crate) fn installed_per_machine_products() -> anyhow::Result<Vec<String>> {
    let upgrade_code = wide(UPGRADE_CODE);
    let mut products = Vec::new();
    for index in 0.. {
        let mut product = [0u16; PRODUCT_CODE_CAPACITY];
        let result = unsafe {
            MsiEnumRelatedProductsW(upgrade_code.as_ptr(), 0, index, product.as_mut_ptr())
        };
        if result == ERROR_NO_MORE_ITEMS {
            break;
        }
        if result != ERROR_SUCCESS {
            bail!("enumerate installed Lumia products (Windows Installer error {result})");
        }
        let product = from_wide(&product);
        let location = product_install_location(&product).unwrap_or_default();
        if is_program_files_install(&location) {
            products.push(product);
        }
    }
    Ok(products)
}

pub(crate) fn uninstall(product_code: &str) -> anyhow::Result<()> {
    let status = Command::new("msiexec.exe")
        .args(["/x", product_code, "/passive", "/norestart"])
        .status()
        .with_context(|| format!("start removal of old Lumia product {product_code}"))?;
    accept_installer_status(status.code(), "remove the old Lumia version")
}

fn product_install_location(product_code: &str) -> anyhow::Result<std::path::PathBuf> {
    let product_code = wide(product_code);
    let mut buffer = vec![0u16; 32_768];
    let mut length = (buffer.len() - 1) as u32;
    let result = unsafe {
        MsiGetProductInfoW(
            product_code.as_ptr(),
            INSTALLPROPERTY_INSTALLLOCATION,
            buffer.as_mut_ptr(),
            &mut length,
        )
    };
    if result != ERROR_SUCCESS {
        bail!("read the old Lumia install location (Windows Installer error {result})");
    }
    Ok(from_wide(&buffer[..length as usize]).into())
}

fn is_program_files_install(path: &Path) -> bool {
    if path.as_os_str().is_empty() {
        return false;
    }
    [
        std::env::var_os("ProgramFiles"),
        std::env::var_os("ProgramFiles(x86)"),
    ]
    .into_iter()
    .flatten()
    .map(std::path::PathBuf::from)
    .any(|root| path_starts_with_case_insensitive(path, &root))
}

fn path_starts_with_case_insensitive(path: &Path, root: &Path) -> bool {
    let path = path.to_string_lossy().replace('/', "\\").to_lowercase();
    let root = root.to_string_lossy().replace('/', "\\").to_lowercase();
    path == root || path.starts_with(&(root.trim_end_matches('\\').to_owned() + "\\"))
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn from_wide(value: &[u16]) -> String {
    let length = value
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(value.len());
    String::from_utf16_lossy(&value[..length])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_prefix_check_is_case_insensitive_and_component_aware() {
        assert!(path_starts_with_case_insensitive(
            Path::new(r"C:\Program Files\Lumia"),
            Path::new(r"c:\program files")
        ));
        assert!(!path_starts_with_case_insensitive(
            Path::new(r"C:\Program Files-old\Lumia"),
            Path::new(r"C:\Program Files")
        ));
    }
}
