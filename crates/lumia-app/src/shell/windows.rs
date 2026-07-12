mod registration;

use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr;

use anyhow::{bail, Context as _};
use windows_sys::Win32::UI::Shell::{
    SHChangeNotify, ShellExecuteW, SHCNE_ASSOCCHANGED, SHCNF_IDLIST,
};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use winreg::{enums::*, RegKey};

use crate::shell::FileAssociationSnapshot;
use registration::{
    build_apply_plan, build_unregister_plan, open_command, RegistryData, RegistryPlan,
};

const ASSOCIATIONS_KEY: &str = "Software\\Lumia\\Associations";
const PROG_ID: &str = "Lumia.Image";

pub(super) fn register(exe_path: &Path) -> anyhow::Result<()> {
    let selected = lumia_core::supported_image_extensions()
        .iter()
        .map(|extension| (*extension).to_string())
        .collect::<BTreeSet<_>>();
    apply(exe_path, &selected)
}

pub(super) fn query(exe_path: &Path) -> anyhow::Result<FileAssociationSnapshot> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let expected_command = open_command(exe_path);
    let configured_key = hkcu.open_subkey(ASSOCIATIONS_KEY).ok();
    let configured = configured_key
        .as_ref()
        .and_then(|key| key.get_value::<u32, _>("Configured").ok())
        == Some(1);
    let selected_extensions = configured_key
        .and_then(|key| key.get_value::<Vec<String>, _>("SelectedExtensions").ok())
        .unwrap_or_default()
        .into_iter()
        .filter(|extension| lumia_core::is_supported_image_extension(extension))
        .collect();

    let registered_extensions = lumia_core::supported_image_extensions()
        .iter()
        .filter(|extension| extension_is_registered(&hkcu, extension, &expected_command))
        .map(|extension| (*extension).to_string())
        .collect();

    Ok(FileAssociationSnapshot {
        configured,
        registered_extensions,
        selected_extensions,
    })
}

pub(super) fn apply(exe_path: &Path, selected_extensions: &BTreeSet<String>) -> anyhow::Result<()> {
    let plan = build_apply_plan(exe_path, selected_extensions);
    let result = apply_registry_plan(&plan);
    notify_associations_changed();
    result
}

pub(super) fn unregister() -> anyhow::Result<()> {
    let result = apply_registry_plan(&build_unregister_plan());
    notify_associations_changed();
    result
}

pub(super) fn open_default_apps_settings() -> anyhow::Result<()> {
    let operation = wide_null("open");
    let uri = wide_null("ms-settings:defaultapps?registeredAppUser=Lumia");
    let result = unsafe {
        ShellExecuteW(
            ptr::null_mut(),
            operation.as_ptr(),
            uri.as_ptr(),
            ptr::null(),
            ptr::null(),
            SW_SHOWNORMAL,
        )
    };
    if result as isize <= 32 {
        bail!("Windows rejected the Default Apps settings request ({result:?})");
    }
    Ok(())
}

fn apply_registry_plan(plan: &RegistryPlan) -> anyhow::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    for value in &plan.set_values {
        let (key, _) = hkcu
            .create_subkey(&value.path)
            .with_context(|| format!("create registry key {}", value.path))?;
        match &value.data {
            RegistryData::String(data) => key.set_value(&value.name, data),
            RegistryData::Dword(data) => key.set_value(&value.name, data),
            RegistryData::MultiString(data) => key.set_value(&value.name, data),
        }
        .with_context(|| format!("set registry value {}\\{}", value.path, value.name))?;
    }

    for value in &plan.delete_values {
        let Ok(key) = hkcu.open_subkey_with_flags(&value.path, KEY_SET_VALUE) else {
            continue;
        };
        if let Err(error) = key.delete_value(&value.name) {
            if error.kind() != ErrorKind::NotFound {
                return Err(error).with_context(|| {
                    format!("delete registry value {}\\{}", value.path, value.name)
                });
            }
        }
    }

    for path in &plan.delete_trees {
        if let Err(error) = hkcu.delete_subkey_all(path) {
            if error.kind() != ErrorKind::NotFound {
                return Err(error).with_context(|| format!("delete registry key {path}"));
            }
        }
    }
    Ok(())
}

fn extension_is_registered(hkcu: &RegKey, extension: &str, expected_command: &str) -> bool {
    let open_with = format!("Software\\Classes\\.{extension}\\OpenWithProgids");
    let context_command =
        format!("Software\\Classes\\SystemFileAssociations\\.{extension}\\shell\\Lumia\\command");
    let capabilities = "Software\\Lumia\\Capabilities\\FileAssociations";
    let supported_types = "Software\\Classes\\Applications\\lumia-app.exe\\SupportedTypes";
    let prog_id_command = "Software\\Classes\\Lumia.Image\\shell\\open\\command";
    let extension_with_dot = format!(".{extension}");

    registry_string(hkcu, &open_with, PROG_ID).is_some()
        && registry_string(hkcu, &context_command, "").as_deref() == Some(expected_command)
        && registry_string(hkcu, prog_id_command, "").as_deref() == Some(expected_command)
        && registry_string(hkcu, capabilities, &extension_with_dot).as_deref() == Some(PROG_ID)
        && registry_string(hkcu, supported_types, &extension_with_dot).is_some()
}

fn registry_string(root: &RegKey, path: &str, name: &str) -> Option<String> {
    root.open_subkey(path).ok()?.get_value(name).ok()
}

fn notify_associations_changed() {
    unsafe {
        SHChangeNotify(
            SHCNE_ASSOCCHANGED as i32,
            SHCNF_IDLIST,
            ptr::null(),
            ptr::null(),
        );
    }
}

fn wide_null(value: &str) -> Vec<u16> {
    std::ffi::OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
