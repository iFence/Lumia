use std::collections::BTreeSet;
use std::path::Path;

const PROG_ID: &str = "Lumia.Image";
const APPLICATION_EXE: &str = "lumia-app.exe";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RegistryData {
    String(String),
    Dword(u32),
    MultiString(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RegistryValue {
    pub(super) path: String,
    pub(super) name: String,
    pub(super) data: RegistryData,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct RegistryPlan {
    pub(super) set_values: Vec<RegistryValue>,
    pub(super) delete_values: Vec<RegistryValue>,
    pub(super) delete_trees: Vec<String>,
}

pub(super) fn build_apply_plan(
    exe_path: &Path,
    selected_extensions: &BTreeSet<String>,
) -> RegistryPlan {
    let mut plan = RegistryPlan::default();
    let command = open_command(exe_path);
    let icon = format!("\"{}\",0", exe_path.to_string_lossy());

    plan.set_string("Software\\Classes\\Lumia.Image", "", "Lumia image");
    plan.set_string("Software\\Classes\\Lumia.Image\\DefaultIcon", "", &icon);
    plan.set_string(
        "Software\\Classes\\Lumia.Image\\shell\\open\\command",
        "",
        &command,
    );
    plan.set_string(
        "Software\\Classes\\Applications\\lumia-app.exe",
        "FriendlyAppName",
        "Lumia",
    );
    plan.set_string(
        "Software\\Classes\\Applications\\lumia-app.exe\\DefaultIcon",
        "",
        &icon,
    );
    plan.set_string(
        "Software\\Classes\\Applications\\lumia-app.exe\\shell\\open\\command",
        "",
        &command,
    );
    plan.set_string("Software\\Lumia\\Capabilities", "ApplicationName", "Lumia");
    plan.set_string(
        "Software\\Lumia\\Capabilities",
        "ApplicationDescription",
        "Fast and lightweight image viewer",
    );
    plan.set_string("Software\\Lumia\\Capabilities", "ApplicationIcon", &icon);
    plan.set_string(
        "Software\\RegisteredApplications",
        "Lumia",
        "Software\\Lumia\\Capabilities",
    );

    for extension in lumia_core::supported_image_extensions() {
        let extension_with_dot = format!(".{extension}");
        let open_with = format!("Software\\Classes\\.{extension}\\OpenWithProgids");
        let supported_types =
            format!("Software\\Classes\\Applications\\{APPLICATION_EXE}\\SupportedTypes");
        let capabilities = "Software\\Lumia\\Capabilities\\FileAssociations";
        let context_menu =
            format!("Software\\Classes\\SystemFileAssociations\\.{extension}\\shell\\Lumia");

        if selected_extensions.contains(*extension) {
            plan.set_string(&open_with, PROG_ID, "");
            plan.set_string(&supported_types, &extension_with_dot, "");
            plan.set_string(capabilities, &extension_with_dot, PROG_ID);
            plan.set_string(&context_menu, "MUIVerb", "Open with Lumia");
            plan.set_string(&context_menu, "Icon", &icon);
            plan.set_string(&format!("{context_menu}\\command"), "", &command);
        } else {
            plan.delete_value(&open_with, PROG_ID);
            plan.delete_value(&supported_types, &extension_with_dot);
            plan.delete_value(capabilities, &extension_with_dot);
            plan.delete_trees.push(context_menu);
        }
    }

    plan.delete_trees
        .push("Software\\Classes\\SystemFileAssociations\\image\\shell\\Lumia".to_string());
    plan.set_dword("Software\\Lumia\\Associations", "Configured", 1);
    plan.set_multi_string(
        "Software\\Lumia\\Associations",
        "SelectedExtensions",
        selected_extensions.iter().cloned().collect(),
    );
    plan
}

pub(super) fn build_unregister_plan() -> RegistryPlan {
    let mut plan = RegistryPlan::default();
    plan.delete_value("Software\\RegisteredApplications", "Lumia");
    plan.delete_trees.extend([
        "Software\\Classes\\Lumia.Image".to_string(),
        "Software\\Classes\\Applications\\lumia-app.exe".to_string(),
        "Software\\Classes\\SystemFileAssociations\\image\\shell\\Lumia".to_string(),
        "Software\\Lumia".to_string(),
    ]);
    for extension in lumia_core::supported_image_extensions() {
        plan.delete_value(
            &format!("Software\\Classes\\.{extension}\\OpenWithProgids"),
            PROG_ID,
        );
        plan.delete_trees.push(format!(
            "Software\\Classes\\SystemFileAssociations\\.{extension}\\shell\\Lumia"
        ));
    }
    plan
}

pub(super) fn open_command(exe_path: &Path) -> String {
    format!("\"{}\" \"%1\"", exe_path.to_string_lossy())
}

impl RegistryPlan {
    fn set_string(&mut self, path: &str, name: &str, data: &str) {
        self.set_values.push(RegistryValue {
            path: path.to_string(),
            name: name.to_string(),
            data: RegistryData::String(data.to_string()),
        });
    }

    fn set_dword(&mut self, path: &str, name: &str, data: u32) {
        self.set_values.push(RegistryValue {
            path: path.to_string(),
            name: name.to_string(),
            data: RegistryData::Dword(data),
        });
    }

    fn set_multi_string(&mut self, path: &str, name: &str, data: Vec<String>) {
        self.set_values.push(RegistryValue {
            path: path.to_string(),
            name: name.to_string(),
            data: RegistryData::MultiString(data),
        });
    }

    fn delete_value(&mut self, path: &str, name: &str) {
        self.delete_values.push(RegistryValue {
            path: path.to_string(),
            name: name.to_string(),
            data: RegistryData::String(String::new()),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selected(values: &[&str]) -> BTreeSet<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn apply_plan_registers_only_selected_extensions() {
        let plan = build_apply_plan(
            Path::new(r"C:\Program Files\Lumia\lumia-app.exe"),
            &selected(&["jpg", "jpeg", "png"]),
        );

        assert!(plan.set_values.iter().any(|value| {
            value.path.ends_with(r".png\OpenWithProgids") && value.name == PROG_ID
        }));
        assert!(plan.delete_values.iter().any(|value| {
            value.path.ends_with(r".gif\OpenWithProgids") && value.name == PROG_ID
        }));
        assert!(plan.set_values.iter().any(|value| {
            value.path.ends_with(r".jpg\shell\Lumia\command")
                && value.data
                    == RegistryData::String(r#""C:\Program Files\Lumia\lumia-app.exe" "%1""#.into())
        }));
    }

    #[test]
    fn apply_plan_records_selection_and_cleans_legacy_menu() {
        let plan = build_apply_plan(Path::new("lumia-app.exe"), &selected(&["png"]));

        assert!(plan.set_values.iter().any(|value| {
            value.path == r"Software\Lumia\Associations"
                && value.name == "SelectedExtensions"
                && value.data == RegistryData::MultiString(vec!["png".into()])
        }));
        assert!(plan
            .delete_trees
            .contains(&r"Software\Classes\SystemFileAssociations\image\shell\Lumia".to_string()));
    }

    #[test]
    fn apply_plan_registers_photoshop_extensions() {
        let plan = build_apply_plan(
            Path::new(r"C:\Program Files\Lumia\lumia-app.exe"),
            &selected(&["psd", "psb"]),
        );

        for extension in ["psd", "psb"] {
            assert!(plan.set_values.iter().any(|value| {
                value
                    .path
                    .ends_with(&format!(r".{extension}\OpenWithProgids"))
                    && value.name == PROG_ID
            }));
            assert!(plan.set_values.iter().any(|value| {
                value.path == r"Software\Lumia\Capabilities\FileAssociations"
                    && value.name == format!(".{extension}")
                    && value.data == RegistryData::String(PROG_ID.into())
            }));
        }
    }

    #[test]
    fn plans_never_modify_windows_user_choice() {
        let apply = build_apply_plan(Path::new("lumia-app.exe"), &selected(&["png"]));
        let unregister = build_unregister_plan();
        let paths = apply
            .set_values
            .iter()
            .chain(&apply.delete_values)
            .chain(&unregister.delete_values)
            .map(|value| value.path.as_str())
            .chain(apply.delete_trees.iter().map(String::as_str))
            .chain(unregister.delete_trees.iter().map(String::as_str));

        assert!(paths
            .filter(|path| path.contains("UserChoice"))
            .next()
            .is_none());
    }

    #[test]
    fn unregister_plan_removes_only_lumia_owned_entries() {
        let plan = build_unregister_plan();
        assert!(plan
            .delete_values
            .iter()
            .all(|value| value.name == PROG_ID || value.name == "Lumia"));
        assert!(plan
            .delete_trees
            .iter()
            .all(|path| path.contains("Lumia") || path.contains("lumia-app.exe")));
    }
}
