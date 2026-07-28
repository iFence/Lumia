use std::collections::HashMap;

use gpui::Context;
use lumia_plugin_api::PluginPermission;

use crate::app::LumiaApp;
use crate::plugin_catalog::{
    load_official_ui_plugin, user_plugin_root, InstalledPlugin, PluginRegistry,
};
use crate::plugin_installation::{install_verified_package, uninstall_plugin};
use crate::plugin_package::{
    inspect_official_package, inspect_packaged_plugin_manifest, validate_compatibility,
    VerifiedPluginPackage,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ManagedPlugin {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) install_directory: String,
    pub(crate) permissions: Vec<PluginPermission>,
}

impl ManagedPlugin {
    fn from_installed(plugin: &InstalledPlugin) -> Self {
        Self {
            id: plugin.manifest.id.clone(),
            name: plugin.manifest.name.clone(),
            version: plugin.manifest.version.clone(),
            install_directory: plugin
                .root
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&plugin.manifest.id)
                .to_string(),
            permissions: plugin.manifest.permissions.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct PendingPluginInstall {
    pub(crate) package: VerifiedPluginPackage,
    pub(crate) name: String,
    pub(crate) permissions: Vec<PluginPermission>,
    pub(crate) installed_version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PluginManagementErrorKind {
    InvalidPackage,
    Incompatible,
    Installation,
    Removal,
    Storage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PluginManagementStatus {
    Idle,
    Inspecting,
    AwaitingConfirmation,
    Installing,
    Installed {
        name: String,
        version: String,
        restart_required: bool,
    },
    Removing {
        plugin_id: String,
    },
    Removed {
        name: String,
        restart_required: bool,
    },
    Error {
        kind: PluginManagementErrorKind,
        message: String,
    },
}

pub(crate) struct PluginManagementState {
    pub(crate) installed: Vec<ManagedPlugin>,
    pub(crate) pending: Option<PendingPluginInstall>,
    pub(crate) status: PluginManagementStatus,
    generation: u64,
}

impl PluginManagementState {
    pub(crate) fn from_registry(registry: &PluginRegistry) -> Self {
        let installed = managed_plugins(registry);
        Self {
            installed,
            pending: None,
            status: PluginManagementStatus::Idle,
            generation: 0,
        }
    }

    fn refresh(&mut self, registry: &PluginRegistry) {
        self.installed = managed_plugins(registry);
    }
    fn record_installed(&mut self, plugin: &InstalledPlugin) {
        let managed = ManagedPlugin::from_installed(plugin);
        self.installed.retain(|existing| existing.id != managed.id);
        self.installed.push(managed);
        self.installed
            .sort_by(|left, right| left.name.cmp(&right.name));
    }

    fn begin_inspection(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.pending = None;
        self.status = PluginManagementStatus::Inspecting;
        self.generation
    }

    fn is_current(&self, generation: u64) -> bool {
        self.generation == generation
    }

    pub(crate) fn cancel_pending(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.pending = None;
        self.status = PluginManagementStatus::Idle;
    }
}

impl LumiaApp {
    pub(crate) fn choose_plugin_package(&mut self, cx: &mut Context<Self>) {
        let handle = self.self_handle.clone();
        cx.spawn(async move |_this, cx| {
            let selected = cx
                .background_executor()
                .spawn(async move {
                    rfd::FileDialog::new()
                        .add_filter("Lumia plugin", &["lumiaplugin"])
                        .pick_file()
                })
                .await;
            let Some(path) = selected else { return };
            let _ = handle.update(cx, |this, cx| {
                this.inspect_plugin_package(path, cx);
            });
        })
        .detach();
    }

    fn inspect_plugin_package(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        let generation = self.plugin_management.begin_inspection();
        let installed_versions = self
            .plugins
            .registry
            .all()
            .map(|plugin| (plugin.manifest.id.clone(), plugin.manifest.version.clone()))
            .collect::<HashMap<_, _>>();
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let package = inspect_official_package(&path, None)?;
                    let installed_version =
                        installed_versions.get(&package.manifest.plugin_id).cloned();
                    validate_compatibility(&package.manifest, installed_version.as_deref())?;
                    let plugin_manifest = inspect_packaged_plugin_manifest(&package)?;
                    Ok::<_, crate::plugin_package::PluginPackageError>((
                        package,
                        plugin_manifest,
                        installed_version,
                    ))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if !this.plugin_management.is_current(generation) {
                    return;
                }
                match result {
                    Ok((package, manifest, installed_version)) => {
                        this.plugin_management.pending = Some(PendingPluginInstall {
                            package,
                            name: manifest.name,
                            permissions: manifest.permissions,
                            installed_version,
                        });
                        this.plugin_management.status =
                            PluginManagementStatus::AwaitingConfirmation;
                    }
                    Err(error) => {
                        this.plugin_management.pending = None;
                        this.plugin_management.status = PluginManagementStatus::Error {
                            kind: package_error_kind(&error),
                            message: error.to_string(),
                        };
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn confirm_plugin_install(&mut self, cx: &mut Context<Self>) {
        let Some(pending) = self.plugin_management.pending.clone() else {
            return;
        };
        let Some(plugin_root) = user_plugin_root() else {
            self.plugin_management.status = PluginManagementStatus::Error {
                kind: PluginManagementErrorKind::Storage,
                message: "the per-user plugin directory is unavailable".into(),
            };
            cx.notify();
            return;
        };
        self.plugin_management.status = PluginManagementStatus::Installing;
        self.plugin_management.generation = self.plugin_management.generation.wrapping_add(1);
        let generation = self.plugin_management.generation;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let outcome = install_verified_package(&pending.package, &plugin_root)?;
                    let installed_root =
                        plugin_root.join(&pending.package.manifest.install_directory);
                    let installed = load_official_ui_plugin(&installed_root).map_err(|error| {
                        crate::plugin_installation::PluginInstallationError::RuntimeValidation(
                            error.to_string(),
                        )
                    })?;
                    Ok::<_, crate::plugin_installation::PluginInstallationError>((
                        outcome, installed,
                    ))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if !this.plugin_management.is_current(generation) {
                    return;
                }
                match result {
                    Ok((outcome, installed)) => {
                        let name = installed.manifest.name.clone();
                        this.plugin_management.record_installed(&installed);
                        this.plugin_management.pending = None;
                        this.plugin_management.status = PluginManagementStatus::Installed {
                            name,
                            version: outcome.installed_version,
                            restart_required: outcome.restart_required,
                        };
                    }
                    Err(error) => {
                        this.plugin_management.status = PluginManagementStatus::Error {
                            kind: PluginManagementErrorKind::Installation,
                            message: error.to_string(),
                        };
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn cancel_plugin_install(&mut self, cx: &mut Context<Self>) {
        self.plugin_management.cancel_pending();
        cx.notify();
    }

    pub(crate) fn remove_managed_plugin(&mut self, plugin_id: String, cx: &mut Context<Self>) {
        let Some(plugin) = self
            .plugin_management
            .installed
            .iter()
            .find(|plugin| plugin.id == plugin_id)
            .cloned()
        else {
            return;
        };
        let Some(plugin_root) = user_plugin_root() else {
            self.plugin_management.status = PluginManagementStatus::Error {
                kind: PluginManagementErrorKind::Storage,
                message: "the per-user plugin directory is unavailable".into(),
            };
            cx.notify();
            return;
        };
        if self
            .plugins
            .active
            .as_ref()
            .is_some_and(|active| active.plugin_id == plugin.id)
        {
            self.plugin_management.status = PluginManagementStatus::Error {
                kind: PluginManagementErrorKind::Removal,
                message: "close the active plugin session before removing it".into(),
            };
            cx.notify();
            return;
        }
        self.plugin_management.generation = self.plugin_management.generation.wrapping_add(1);
        let generation = self.plugin_management.generation;
        self.plugin_management.status = PluginManagementStatus::Removing {
            plugin_id: plugin.id.clone(),
        };
        cx.notify();
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    uninstall_plugin(&plugin_root, &plugin.install_directory)
                        .map(|outcome| (plugin, outcome))
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if !this.plugin_management.is_current(generation) {
                    return;
                }
                match result {
                    Ok((plugin, outcome)) => {
                        this.plugins.registry.remove(&plugin.id);
                        this.plugin_management.refresh(&this.plugins.registry);
                        this.plugin_management.status = PluginManagementStatus::Removed {
                            name: plugin.name,
                            restart_required: outcome.restart_required,
                        };
                    }
                    Err(error) => {
                        this.plugin_management.status = PluginManagementStatus::Error {
                            kind: PluginManagementErrorKind::Removal,
                            message: error.to_string(),
                        };
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

fn managed_plugins(registry: &PluginRegistry) -> Vec<ManagedPlugin> {
    let Some(user_root) = user_plugin_root() else {
        return Vec::new();
    };
    let mut plugins = registry
        .all()
        .filter(|plugin| plugin.root.parent() == Some(user_root.as_path()))
        .map(ManagedPlugin::from_installed)
        .collect::<Vec<_>>();
    plugins.sort_by(|left, right| left.name.cmp(&right.name));
    plugins
}

fn package_error_kind(
    error: &crate::plugin_package::PluginPackageError,
) -> PluginManagementErrorKind {
    use crate::plugin_package::PluginPackageError as Error;
    match error {
        Error::IncompatiblePlatform { .. }
        | Error::IncompatibleArchitecture { .. }
        | Error::IncompatiblePluginApi { .. }
        | Error::IncompatibleLumiaVersion { .. }
        | Error::DowngradeBlocked { .. } => PluginManagementErrorKind::Incompatible,
        Error::Io(_) => PluginManagementErrorKind::Storage,
        _ => PluginManagementErrorKind::InvalidPackage,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspection_generation_rejects_stale_completion() {
        let registry = PluginRegistry::default();
        let mut state = PluginManagementState::from_registry(&registry);
        let first = state.begin_inspection();
        let second = state.begin_inspection();

        assert!(!state.is_current(first));
        assert!(state.is_current(second));
        assert_eq!(state.status, PluginManagementStatus::Inspecting);
    }

    #[test]
    fn cancel_clears_pending_operation_state() {
        let registry = PluginRegistry::default();
        let mut state = PluginManagementState::from_registry(&registry);
        state.begin_inspection();
        state.cancel_pending();

        assert!(state.pending.is_none());
        assert_eq!(state.status, PluginManagementStatus::Idle);
    }

    #[test]
    fn invalid_signature_is_a_stable_invalid_package_category() {
        assert_eq!(
            package_error_kind(&crate::plugin_package::PluginPackageError::InvalidSignature),
            PluginManagementErrorKind::InvalidPackage
        );
    }
}
