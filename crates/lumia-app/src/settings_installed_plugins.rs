//! "Installed" tab of the Plugins settings page: install-from-file, pending
//! confirmation card, operation status banner, and the installed plugin list.
//! Split out of `settings_plugins.rs` to keep each module under the project's
//! 500-line limit.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, px, rgb, AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled,
};
use lumia_core::Language;
use lumia_plugin_api::PluginPermission;

use crate::app::LumiaApp;
use crate::i18n::{tr, TextKey};
use crate::palette::Palette;
use crate::plugin_management::{PluginManagementErrorKind, PluginManagementStatus};
use crate::widgets::settings_action_button;

impl LumiaApp {
    pub(crate) fn render_installed_plugins(
        &self,
        palette: Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let language = self.settings.language;
        let busy = matches!(
            self.plugin_management.status,
            PluginManagementStatus::Inspecting
                | PluginManagementStatus::Installing
                | PluginManagementStatus::Removing { .. }
        );
        let choose_handle = self.self_handle.clone();
        let mut content = div().flex().flex_col().gap_4();

        content = content.child(settings_action_button(
            "install-plugin-package",
            tr(language, TextKey::InstallPlugin),
            true,
            busy,
            move |_, _, cx| {
                let _ = choose_handle.update(cx, |this, cx| {
                    this.choose_plugin_package(cx);
                });
            },
        ));
        content = content.children(self.render_plugin_operation_status(palette));
        content = content.children(self.render_pending_plugin_confirmation(palette, cx));
        content = content.child(
            div()
                .pt_2()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .child(tr(language, TextKey::InstalledPlugins)),
        );

        if self.plugin_management.installed.is_empty() {
            content = content.child(
                div()
                    .p_4()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(palette.border))
                    .text_sm()
                    .text_color(rgb(palette.muted_text))
                    .child(tr(language, TextKey::NoInstalledPlugins)),
            );
        } else {
            for (index, plugin) in self.plugin_management.installed.iter().enumerate() {
                let plugin_id = plugin.id.clone();
                let remove_handle = self.self_handle.clone();
                let removing_this = matches!(
                    &self.plugin_management.status,
                    PluginManagementStatus::Removing { plugin_id: removing } if removing == &plugin.id
                );
                let permission_summary =
                    permission_summary(language, &plugin.permissions, TextKey::NoPermissions);
                let remove_button = div()
                    .id(format!("remove-plugin-{index}"))
                    .h(px(30.0))
                    .px_3()
                    .flex()
                    .items_center()
                    .rounded_sm()
                    .border_1()
                    .border_color(rgb(palette.border))
                    .text_sm()
                    .text_color(rgb(if busy {
                        palette.muted_text
                    } else {
                        palette.error_text
                    }))
                    .when(!busy, |button| {
                        button
                            .cursor_pointer()
                            .hover(move |style| style.bg(rgb(palette.button_hover)))
                            .on_click(move |_, _, cx| {
                                let plugin_id = plugin_id.clone();
                                let _ = remove_handle.update(cx, |this, cx| {
                                    this.remove_managed_plugin(plugin_id, cx);
                                });
                            })
                    })
                    .child(if removing_this {
                        "..."
                    } else {
                        tr(language, TextKey::Remove)
                    });
                content = content.child(
                    div()
                        .p_4()
                        .rounded_md()
                        .border_1()
                        .border_color(rgb(palette.border))
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(FontWeight::BOLD)
                                        .child(format!("{}  {}", plugin.name, plugin.version)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(palette.muted_text))
                                        .child(plugin.id.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(palette.muted_text))
                                        .child(permission_summary),
                                ),
                        )
                        .child(remove_button),
                );
            }
        }
        Some(content.into_any_element())
    }

    fn render_pending_plugin_confirmation(
        &self,
        palette: Palette,
        _cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let pending = self.plugin_management.pending.as_ref()?;
        let language = self.settings.language;
        let install_handle = self.self_handle.clone();
        let cancel_handle = self.self_handle.clone();
        let version_context = pending
            .installed_version
            .as_ref()
            .map(|installed| format!("{installed} → {}", pending.package.manifest.version))
            .unwrap_or_else(|| pending.package.manifest.version.clone());
        let permissions =
            permission_summary(language, &pending.permissions, TextKey::NoPermissions);

        Some(
            div()
                .id("plugin-install-confirmation")
                .p_4()
                .rounded_md()
                .border_1()
                .border_color(rgb(palette.accent))
                .bg(rgb(palette.sidebar_bg))
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::BOLD)
                        .child(format!("{}  {version_context}", pending.name)),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(palette.muted_text))
                        .child(format!(
                            "{}: {permissions}",
                            tr(language, TextKey::Permissions)
                        )),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(settings_action_button(
                            "confirm-plugin-install",
                            tr(language, TextKey::Install),
                            true,
                            false,
                            move |_, _, cx| {
                                let _ = install_handle.update(cx, |this, cx| {
                                    this.confirm_plugin_install(cx);
                                });
                            },
                        ))
                        .child(settings_action_button(
                            "cancel-plugin-install",
                            tr(language, TextKey::Cancel),
                            false,
                            false,
                            move |_, _, cx| {
                                let _ = cancel_handle.update(cx, |this, cx| {
                                    this.cancel_plugin_install(cx);
                                });
                            },
                        )),
                )
                .into_any_element(),
        )
    }

    fn render_plugin_operation_status(&self, palette: Palette) -> Option<AnyElement> {
        let language = self.settings.language;
        let (message, is_error) = match &self.plugin_management.status {
            PluginManagementStatus::Idle | PluginManagementStatus::AwaitingConfirmation => {
                return None;
            }
            PluginManagementStatus::Inspecting => {
                (tr(language, TextKey::InspectingPlugin).to_string(), false)
            }
            PluginManagementStatus::Installing => {
                (tr(language, TextKey::InstallingPlugin).to_string(), false)
            }
            PluginManagementStatus::Installed {
                name,
                version,
                restart_required,
            } => (
                format!(
                    "{}: {name} {version}{}",
                    tr(language, TextKey::PluginInstalled),
                    restart_suffix(language, *restart_required)
                ),
                false,
            ),
            PluginManagementStatus::Removing { .. } => ("...".into(), false),
            PluginManagementStatus::Removed {
                name,
                restart_required,
            } => (
                format!(
                    "{}: {name}{}",
                    tr(language, TextKey::PluginRemoved),
                    restart_suffix(language, *restart_required)
                ),
                false,
            ),
            PluginManagementStatus::Error { kind, message } => (
                format!(
                    "{} [{}]: {message}",
                    tr(language, TextKey::PluginOperationFailed),
                    error_kind_label(*kind)
                ),
                true,
            ),
        };
        Some(
            div()
                .p_3()
                .rounded_md()
                .bg(rgb(palette.sidebar_bg))
                .text_sm()
                .text_color(rgb(if is_error {
                    palette.error_text
                } else {
                    palette.text
                }))
                .child(message)
                .into_any_element(),
        )
    }
}

fn restart_suffix(language: Language, required: bool) -> String {
    required
        .then(|| format!(" — {}", tr(language, TextKey::RestartRequired)))
        .unwrap_or_default()
}

pub(crate) fn permission_summary(
    language: Language,
    permissions: &[PluginPermission],
    empty_key: TextKey,
) -> String {
    if permissions.is_empty() {
        return tr(language, empty_key).into();
    }
    permissions
        .iter()
        .map(|permission| match permission {
            PluginPermission::ReadInputPath => "read_input_path".into(),
            PluginPermission::WriteTemporaryOutput => "write_temporary_output".into(),
            PluginPermission::Network => "network".into(),
            PluginPermission::ReadConfigSecret(name) => format!("read_config_secret:{name}"),
        })
        .collect::<Vec<String>>()
        .join(", ")
}

fn error_kind_label(kind: PluginManagementErrorKind) -> &'static str {
    match kind {
        PluginManagementErrorKind::InvalidPackage => "invalid_package",
        PluginManagementErrorKind::Incompatible => "incompatible",
        PluginManagementErrorKind::Installation => "installation",
        PluginManagementErrorKind::Removal => "removal",
        PluginManagementErrorKind::Storage => "storage",
    }
}
