use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui::{Context, Window};
use lumia_plugin_api::{
    CanvasToolSettings, CanvasToolState, DocumentContext, PanelModel, PluginIcon, PluginManifest,
    UiActivateParams, UiEventParams, UiSessionResult, UiUpdateResult, UiValue,
};
use lumia_plugin_host::{
    validate_canvas_state, validate_panel_model, validate_ui_session, PluginProcess,
};

use crate::app::LumiaApp;
use crate::plugin_catalog::{InstalledPlugin, PluginRegistry};

pub(crate) struct PluginUiState {
    pub(crate) registry: PluginRegistry,
    pub(crate) active: Option<ActivePluginSession>,
    pub(crate) feedback: Option<String>,
    pub(crate) generation: u64,
}

pub(crate) struct ActivePluginSession {
    pub(crate) plugin_id: String,
    pub(crate) manifest: PluginManifest,
    pub(crate) session_id: String,
    pub(crate) panel: PanelModel,
    pub(crate) canvas: Option<CanvasToolState>,
    pub(crate) process: Arc<Mutex<PluginProcess>>,
    pub(crate) busy: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct PluginMenuItem {
    pub(crate) plugin_id: String,
    pub(crate) command_id: String,
    pub(crate) label: String,
    pub(crate) icon: PluginIcon,
    pub(crate) icon_path: Option<std::path::PathBuf>,
    pub(crate) enabled: bool,
}

/// Structured view of the active canvas tool's settings, parsed out of the
/// plugin's `CanvasToolState` and ready for host-side placement.
#[derive(Debug, Clone, Copy)]
pub(crate) enum ActiveToolSettings {
    Text {
        font_size: f32,
        color: u32,
        opacity: f32,
    },
    Rectangle {
        stroke_width: f32,
        color: u32,
        opacity: f32,
    },
    NumberedStep {
        size: f32,
        color: u32,
        opacity: f32,
    },
}

impl PluginUiState {
    pub(crate) fn new() -> Self {
        Self {
            registry: PluginRegistry::discover(),
            active: None,
            feedback: None,
            generation: 0,
        }
    }

    pub(crate) fn context_menu_items(
        &self,
        language: &str,
        has_document: bool,
        canvas_available: bool,
    ) -> Vec<PluginMenuItem> {
        let mut items = Vec::new();
        for plugin in self.registry.ui_plugins() {
            for menu in &plugin.manifest.contributions.viewer_context_menu {
                let Some(command) = plugin
                    .manifest
                    .contributions
                    .commands
                    .iter()
                    .find(|command| command.id == menu.command_id)
                else {
                    continue;
                };
                items.push((
                    menu.group,
                    menu.order,
                    PluginMenuItem {
                        plugin_id: plugin.manifest.id.clone(),
                        command_id: command.id.clone(),
                        label: command.label.resolve(language).to_string(),
                        icon: command.icon.clone(),
                        icon_path: match &command.icon {
                            PluginIcon::Asset(asset_id) => plugin.asset_path(asset_id),
                            _ => None,
                        },
                        enabled: (!command.requires_document || has_document)
                            && (!plugin
                                .manifest
                                .capabilities
                                .contains(&lumia_plugin_api::PluginCapability::CanvasOverlay)
                                || canvas_available),
                    },
                ));
            }
        }
        items.sort_by_key(|(group, order, _)| (*group, *order));
        items.into_iter().map(|(_, _, item)| item).collect()
    }

    pub(crate) fn active_tool_settings(&self) -> Option<ActiveToolSettings> {
        let canvas = self.active.as_ref()?.canvas.as_ref()?;
        match &canvas.settings {
            CanvasToolSettings::Text {
                font_size,
                color,
                opacity,
            } => Some(ActiveToolSettings::Text {
                font_size: *font_size,
                color: parse_hex_color(color)?,
                opacity: *opacity,
            }),
            CanvasToolSettings::Rectangle {
                stroke_width,
                color,
                opacity,
            } => Some(ActiveToolSettings::Rectangle {
                stroke_width: *stroke_width,
                color: parse_hex_color(color)?,
                opacity: *opacity,
            }),
            CanvasToolSettings::NumberedStep {
                size,
                color,
                opacity,
            } => Some(ActiveToolSettings::NumberedStep {
                size: *size,
                color: parse_hex_color(color)?,
                opacity: *opacity,
            }),
        }
    }

    pub(crate) fn is_text_tool_active(&self) -> bool {
        self.active
            .as_ref()
            .and_then(|session| session.canvas.as_ref())
            .is_some_and(|canvas| {
                matches!(
                    canvas.settings,
                    CanvasToolSettings::Text { .. }
                )
            })
    }

    pub(crate) fn active_asset_path(&self, asset_id: &str) -> Option<std::path::PathBuf> {
        let active = self.active.as_ref()?;
        let plugin = self.registry.get(&active.plugin_id)?;
        plugin.asset_path(asset_id)
    }
}

impl LumiaApp {
    pub(crate) fn activate_plugin_command(
        &mut self,
        plugin_id: String,
        command_id: String,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.editing.mode.is_some() || self.ui.show_settings_panel {
            return;
        }
        let Some(plugin) = self.plugins.registry.get(&plugin_id).cloned() else {
            return;
        };
        if plugin
            .manifest
            .capabilities
            .contains(&lumia_plugin_api::PluginCapability::CanvasOverlay)
            && !self.plugin_canvas_available()
        {
            return;
        }
        let Some(panel) = plugin
            .manifest
            .contributions
            .right_panels
            .iter()
            .find(|panel| panel.command_id == command_id)
            .cloned()
        else {
            return;
        };
        let Some((width, height)) = self.viewer.display_dimensions() else {
            return;
        };

        self.stop_slideshow(cx);
        self.close_plugin_session(cx);
        self.ui.context_menu_position = None;
        self.plugins.generation = self.plugins.generation.wrapping_add(1);
        let generation = self.plugins.generation;
        self.plugins.feedback = None;
        let document = DocumentContext {
            document_id: self
                .image_path()
                .map(document_id)
                .unwrap_or_else(|| "document".to_string()),
            width,
            height,
            rotation_quarter_turns: self.viewer.rotation_quarter_turns(),
        };

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { start_ui_session(plugin, panel.id, document) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.plugins.generation != generation {
                    return;
                }
                match result {
                    Ok(active) => {
                        this.plugins.active = Some(active);
                        this.annotations.reset();
                        this.clear_transient_annotation_ui(cx);
                    }
                    Err(error) => {
                        this.plugins.feedback = Some(format!("{error:#}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn dispatch_plugin_ui_event(
        &mut self,
        control_id: String,
        value: UiValue,
        cx: &mut Context<Self>,
    ) {
        let Some(active) = self.plugins.active.as_mut() else {
            return;
        };
        if active.busy {
            return;
        }
        active.busy = true;
        let session_id = active.session_id.clone();
        let process = Arc::clone(&active.process);
        let plugin_id = active.plugin_id.clone();
        let manifest = active.manifest.clone();
        let generation = self.plugins.generation;

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let mut process = process
                        .lock()
                        .map_err(|_| anyhow::anyhow!("plugin process lock is poisoned"))?;
                    let update = process
                        .request_with_timeout::<_, UiUpdateResult>(
                            "ui.event",
                            UiEventParams {
                                session_id,
                                control_id,
                                value,
                            },
                            Duration::from_secs(5),
                        )
                        .map_err(anyhow::Error::from)?;
                    if let Err(error) = validate_panel_model(&manifest, &update.panel)
                        .and_then(|_| validate_canvas_state(&manifest, update.canvas.as_ref()))
                    {
                        process.terminate();
                        return Err(error.into());
                    }
                    Ok::<UiUpdateResult, anyhow::Error>(update)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.plugins.generation != generation {
                    return;
                }
                let Some(active) = this.plugins.active.as_mut() else {
                    return;
                };
                if active.plugin_id != plugin_id {
                    return;
                }
                active.busy = false;
                match result {
                    Ok(update) => {
                        let tool_changed = match (&active.canvas, &update.canvas) {
                            (Some(old), Some(new)) => std::mem::discriminant(&old.settings)
                                != std::mem::discriminant(&new.settings),
                            (None, Some(_)) | (Some(_), None) => true,
                            (None, None) => false,
                        };
                        active.panel = update.panel;
                        active.canvas = update.canvas;
                        if tool_changed {
                            this.clear_transient_annotation_ui(cx);
                        }
                    }
                    Err(error) => {
                        active.busy = true;
                        this.plugins.feedback = Some(format!("{error:#}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn close_plugin_session(&mut self, cx: &mut Context<Self>) {
        self.plugins.generation = self.plugins.generation.wrapping_add(1);
        let Some(active) = self.plugins.active.take() else {
            return;
        };
        self.annotations.reset();
        self.clear_transient_annotation_ui(cx);
        let process = active.process;
        let session_id = active.session_id;
        cx.background_executor()
            .spawn(async move {
                if let Ok(mut process) = process.lock() {
                    let _ = process.request_with_timeout::<_, lumia_plugin_api::EmptyResult>(
                        "ui.close",
                        lumia_plugin_api::UiCloseParams { session_id },
                        Duration::from_secs(5),
                    );
                    let _ = process.request_with_timeout::<_, lumia_plugin_api::EmptyResult>(
                        "plugin.shutdown",
                        lumia_plugin_api::EmptyResult::default(),
                        Duration::from_secs(5),
                    );
                }
            })
            .detach();
        cx.notify();
    }
}

fn start_ui_session(
    plugin: InstalledPlugin,
    panel_id: String,
    document: DocumentContext,
) -> anyhow::Result<ActivePluginSession> {
    let mut manifest: PluginManifest = plugin.manifest.clone();
    manifest.entry = plugin.entry_path();
    let mut process = PluginProcess::spawn(&manifest)?;
    process.initialize_for(&manifest)?;
    let session: UiSessionResult = process.request_with_timeout(
        "ui.activate",
        UiActivateParams {
            contribution_id: panel_id.clone(),
            document,
        },
        Duration::from_secs(5),
    )?;
    validate_ui_session(&manifest, &session)?;
    Ok(ActivePluginSession {
        plugin_id: manifest.id.clone(),
        manifest,
        session_id: session.session_id,
        panel: session.panel,
        canvas: session.canvas,
        process: Arc::new(Mutex::new(process)),
        busy: false,
    })
}

fn document_id(path: &std::path::Path) -> String {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn parse_hex_color(value: &str) -> Option<u32> {
    let value = value.strip_prefix('#')?;
    (value.len() == 6)
        .then(|| u32::from_str_radix(value, 16).ok())
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_colors_parse_only_six_digit_hex() {
        assert_eq!(parse_hex_color("#ff3b30"), Some(0xff3b30));
        assert_eq!(parse_hex_color("ff3b30"), None);
        assert_eq!(parse_hex_color("#bad"), None);
    }

    #[test]
    fn absent_plugins_contribute_no_context_menu_items() {
        let state = PluginUiState {
            registry: PluginRegistry::default(),
            active: None,
            feedback: None,
            generation: 0,
        };
        assert!(state.context_menu_items("en", true, true).is_empty());
    }
}
