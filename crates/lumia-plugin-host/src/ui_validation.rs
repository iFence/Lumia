use std::collections::HashSet;
use std::path::{Component, Path};

use lumia_plugin_api::{
    CanvasToolKind, CanvasToolSettings, CanvasToolState, LocalizedText, PanelControl, PanelModel,
    PluginCapability, PluginIcon, PluginManifest, UiSessionResult,
};

use crate::{PluginHostError, Result};

const MAX_COMMANDS: usize = 32;
const MAX_MENU_ITEMS: usize = 32;
const MAX_PANELS: usize = 8;
const MAX_CANVAS_TOOLS: usize = 16;
const MAX_ASSETS: usize = 64;
const MAX_PANEL_SECTIONS: usize = 16;
const MAX_PANEL_CONTROLS: usize = 96;
const MAX_SELECT_OPTIONS: usize = 32;
const MAX_TEXT_BYTES: usize = 4 * 1024;

pub fn validate_ui_manifest(manifest: &PluginManifest) -> Result<()> {
    let contributions = &manifest.contributions;
    validate_count("commands", contributions.commands.len(), MAX_COMMANDS)?;
    validate_count(
        "context menu contributions",
        contributions.viewer_context_menu.len(),
        MAX_MENU_ITEMS,
    )?;
    validate_count("right panels", contributions.right_panels.len(), MAX_PANELS)?;
    validate_count(
        "canvas tools",
        contributions.canvas_tools.len(),
        MAX_CANVAS_TOOLS,
    )?;
    validate_count("assets", manifest.assets.len(), MAX_ASSETS)?;

    let has_contributions = !contributions.commands.is_empty()
        || !contributions.viewer_context_menu.is_empty()
        || !contributions.right_panels.is_empty()
        || !contributions.canvas_tools.is_empty();
    if has_contributions
        && !manifest
            .capabilities
            .contains(&PluginCapability::UiContributions)
    {
        return Err(PluginHostError::MissingCapability(
            PluginCapability::UiContributions,
        ));
    }
    if !contributions.canvas_tools.is_empty()
        && !manifest
            .capabilities
            .contains(&PluginCapability::CanvasOverlay)
    {
        return Err(PluginHostError::MissingCapability(
            PluginCapability::CanvasOverlay,
        ));
    }

    let mut ids = HashSet::new();
    for command in &contributions.commands {
        validate_unique_id(&command.id, &mut ids)?;
        validate_localized_text(&command.label)?;
        validate_icon(manifest, &command.icon)?;
    }
    for menu in &contributions.viewer_context_menu {
        validate_unique_id(&menu.id, &mut ids)?;
        require_command(manifest, &menu.id, &menu.command_id)?;
    }
    for panel in &contributions.right_panels {
        validate_unique_id(&panel.id, &mut ids)?;
        validate_localized_text(&panel.title)?;
        require_command(manifest, &panel.id, &panel.command_id)?;
    }
    for tool in &contributions.canvas_tools {
        validate_unique_id(&tool.id, &mut ids)?;
        validate_localized_text(&tool.label)?;
        validate_icon(manifest, &tool.icon)?;
    }

    let mut asset_ids = HashSet::new();
    for asset in &manifest.assets {
        validate_id(&asset.id)?;
        if !asset_ids.insert(asset.id.as_str()) {
            return invalid_manifest(format!("duplicate asset id {}", asset.id));
        }
        validate_package_path(&asset.path).map_err(|_| {
            PluginHostError::InvalidManifest(format!("asset {} has an unsafe path", asset.id))
        })?;
        if asset.media_type != "image/svg+xml" {
            return invalid_manifest(format!(
                "asset {} uses unsupported media type {}",
                asset.id, asset.media_type
            ));
        }
        if asset.sha256.len() != 64 || !asset.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return invalid_manifest(format!("asset {} has an invalid sha256", asset.id));
        }
    }
    validate_package_path(&manifest.entry)
        .map_err(|_| PluginHostError::InvalidManifest("plugin entry has an unsafe path".into()))?;
    Ok(())
}

pub fn validate_panel_model(manifest: &PluginManifest, panel: &PanelModel) -> Result<()> {
    validate_localized_text(&panel.title).map_err(as_ui_model)?;
    if panel.sections.len() > MAX_PANEL_SECTIONS {
        return invalid_ui("too many panel sections");
    }
    let control_count = panel
        .sections
        .iter()
        .map(|section| section.controls.len())
        .sum::<usize>();
    if control_count > MAX_PANEL_CONTROLS {
        return invalid_ui("too many panel controls");
    }

    let mut ids = HashSet::new();
    for section in &panel.sections {
        validate_id(&section.id).map_err(as_ui_model)?;
        if !ids.insert(section.id.as_str()) {
            return invalid_ui(format!("duplicate panel id {}", section.id));
        }
        if let Some(title) = &section.title {
            validate_localized_text(title).map_err(as_ui_model)?;
        }
        for control in &section.controls {
            validate_control(manifest, control, &mut ids)?;
        }
    }
    Ok(())
}

pub fn validate_ui_session(manifest: &PluginManifest, session: &UiSessionResult) -> Result<()> {
    validate_id(&session.session_id).map_err(as_ui_model)?;
    validate_panel_model(manifest, &session.panel)?;
    validate_canvas_state(manifest, session.canvas.as_ref())
}

pub fn validate_canvas_state(
    manifest: &PluginManifest,
    state: Option<&CanvasToolState>,
) -> Result<()> {
    let Some(state) = state else {
        return Ok(());
    };
    validate_id(&state.tool_id).map_err(as_ui_model)?;
    let Some(tool) = manifest
        .contributions
        .canvas_tools
        .iter()
        .find(|tool| tool.id == state.tool_id)
    else {
        return invalid_ui(format!("unknown canvas tool {}", state.tool_id));
    };
    match (&tool.kind, &state.settings) {
        (
            CanvasToolKind::IconStamp,
            CanvasToolSettings::IconStamp {
                asset_id,
                size,
                color,
                opacity,
            },
        ) => {
            if !manifest.assets.iter().any(|asset| asset.id == *asset_id) {
                return invalid_ui(format!("unknown icon asset {asset_id}"));
            }
            if !size.is_finite() || !(1.0..=4096.0).contains(size) {
                return invalid_ui("invalid icon stamp size");
            }
            if !opacity.is_finite() || !(0.0..=1.0).contains(opacity) {
                return invalid_ui("invalid icon stamp opacity");
            }
            if !valid_color(color) {
                return invalid_ui("invalid icon stamp color");
            }
            Ok(())
        }
        _ => invalid_ui(format!(
            "canvas state does not match tool kind for {}",
            state.tool_id
        )),
    }
}

fn validate_control<'a>(
    manifest: &PluginManifest,
    control: &'a PanelControl,
    ids: &mut HashSet<&'a str>,
) -> Result<()> {
    validate_id(control.id()).map_err(as_ui_model)?;
    if !ids.insert(control.id()) {
        return invalid_ui(format!("duplicate panel id {}", control.id()));
    }
    match control {
        PanelControl::Button { label, icon, .. } => {
            validate_localized_text(label).map_err(as_ui_model)?;
            validate_icon(manifest, icon).map_err(as_ui_model)
        }
        PanelControl::Toggle { label, .. } | PanelControl::Color { label, .. } => {
            validate_localized_text(label).map_err(as_ui_model)?;
            if let PanelControl::Color { value, .. } = control {
                if !valid_color(value) {
                    return invalid_ui(format!("invalid color value for {}", control.id()));
                }
            }
            Ok(())
        }
        PanelControl::Select {
            label,
            options,
            selected,
            ..
        } => {
            validate_localized_text(label).map_err(as_ui_model)?;
            if options.is_empty() || options.len() > MAX_SELECT_OPTIONS {
                return invalid_ui(format!("invalid option count for {}", control.id()));
            }
            let mut values = HashSet::new();
            for option in options {
                validate_id(&option.value).map_err(as_ui_model)?;
                validate_localized_text(&option.label).map_err(as_ui_model)?;
                if !values.insert(option.value.as_str()) {
                    return invalid_ui(format!("duplicate option {}", option.value));
                }
                if let Some(icon) = &option.icon {
                    validate_icon(manifest, icon).map_err(as_ui_model)?;
                }
            }
            if !values.contains(selected.as_str()) {
                return invalid_ui(format!("unknown selected option {selected}"));
            }
            Ok(())
        }
        PanelControl::Slider {
            label,
            value,
            min,
            max,
            step,
            ..
        } => {
            validate_localized_text(label).map_err(as_ui_model)?;
            if ![value, min, max, step]
                .iter()
                .all(|number| number.is_finite())
                || min > max
                || *step <= 0.0
                || value < min
                || value > max
            {
                return invalid_ui(format!("invalid slider range for {}", control.id()));
            }
            Ok(())
        }
        PanelControl::Text { label, value, .. } => {
            validate_localized_text(label).map_err(as_ui_model)?;
            if value.len() > MAX_TEXT_BYTES {
                return invalid_ui(format!("text value for {} is too long", control.id()));
            }
            Ok(())
        }
    }
}

fn validate_icon(manifest: &PluginManifest, icon: &PluginIcon) -> Result<()> {
    if let PluginIcon::Asset(asset_id) = icon {
        validate_id(asset_id)?;
        if !manifest.assets.iter().any(|asset| asset.id == *asset_id) {
            return invalid_manifest(format!("unknown icon asset {asset_id}"));
        }
    }
    Ok(())
}

fn validate_localized_text(text: &LocalizedText) -> Result<()> {
    if text.fallback.is_empty() || text.fallback.len() > 256 || text.translations.len() > 16 {
        return invalid_manifest("invalid localized text".into());
    }
    if text
        .translations
        .iter()
        .any(|(language, value)| language.is_empty() || language.len() > 32 || value.len() > 256)
    {
        return invalid_manifest("invalid localized translation".into());
    }
    Ok(())
}

fn require_command(manifest: &PluginManifest, id: &str, command_id: &str) -> Result<()> {
    if !manifest
        .contributions
        .commands
        .iter()
        .any(|command| command.id == command_id)
    {
        return invalid_manifest(format!(
            "contribution {id} references unknown command {command_id}"
        ));
    }
    Ok(())
}

fn validate_unique_id<'a>(id: &'a str, ids: &mut HashSet<&'a str>) -> Result<()> {
    validate_id(id)?;
    if !ids.insert(id) {
        return invalid_manifest(format!("duplicate contribution id {id}"));
    }
    Ok(())
}

fn validate_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 96
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return invalid_manifest(format!("invalid contribution id {id:?}"));
    }
    Ok(())
}

fn validate_package_path(path: &Path) -> std::result::Result<(), ()> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(());
    }
    Ok(())
}

fn validate_count(name: &str, count: usize, maximum: usize) -> Result<()> {
    if count > maximum {
        return invalid_manifest(format!("too many {name}"));
    }
    Ok(())
}

fn valid_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn invalid_manifest<T>(message: String) -> Result<T> {
    Err(PluginHostError::InvalidManifest(message))
}

fn invalid_ui<T>(message: impl Into<String>) -> Result<T> {
    Err(PluginHostError::InvalidUiModel(message.into()))
}

fn as_ui_model(error: PluginHostError) -> PluginHostError {
    PluginHostError::InvalidUiModel(error.to_string())
}
