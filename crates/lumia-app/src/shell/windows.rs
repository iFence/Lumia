use std::path::Path;

use winreg::{enums::*, RegKey};

pub(super) fn register(exe_path: &Path) -> anyhow::Result<()> {
    let command = format!("\"{}\" \"%1\"", exe_path.to_string_lossy());
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let classes = hkcu.create_subkey("Software\\Classes")?.0;

    let (progid_key, _) = classes.create_subkey("Lumia.Image\\shell\\open\\command")?;
    progid_key.set_value("", &command)?;

    let (system_key, _) =
        classes.create_subkey("SystemFileAssociations\\image\\shell\\Lumia\\command")?;
    system_key.set_value("", &command)?;

    for extension in lumia_core::supported_image_extensions() {
        let path = format!(".{extension}\\OpenWithProgids");
        if let Ok((extension_key, _)) = classes.create_subkey(path) {
            let _ = extension_key.set_value("Lumia.Image", &"");
        }
    }

    Ok(())
}

pub(super) fn unregister() -> anyhow::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let classes = hkcu.open_subkey_with_flags("Software\\Classes", KEY_ALL_ACCESS)?;

    let _ = classes.delete_subkey_all("Lumia.Image");
    let _ = classes.delete_subkey_all("SystemFileAssociations\\image\\shell\\Lumia");
    for extension in lumia_core::supported_image_extensions() {
        let path = format!(".{extension}\\OpenWithProgids");
        if let Ok(extension_key) = classes.open_subkey_with_flags(path, KEY_ALL_ACCESS) {
            let _ = extension_key.delete_value("Lumia.Image");
        }
    }

    Ok(())
}
