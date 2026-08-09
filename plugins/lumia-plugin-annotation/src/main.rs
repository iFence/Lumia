use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};

use lumia_plugin_api::{
    CanvasOperationCommittedParams, CanvasToolContribution, CanvasToolKind, CanvasToolSettings,
    CanvasToolState, CapabilitiesResult, CommandContribution, EmptyResult, InitializeResult,
    JsonRpcRequest, JsonRpcResponse, LocalizedText, MenuContribution, PanelContribution,
    PanelControl, PanelModel, PanelOption, PanelSection, PluginCapability, PluginContributions,
    PluginIcon, PluginManifest, UiActivateParams, UiEventParams, UiSessionResult, UiUpdateResult,
    UiValue, PROTOCOL_VERSION,
};
use serde_json::json;

const SESSION_ID: &str = "annotation-session";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Tool {
    Text,
    Rectangle,
    NumberedStep,
}

fn tool_name(tool: Tool) -> &'static str {
    match tool {
        Tool::Text => "text",
        Tool::Rectangle => "rectangle",
        Tool::NumberedStep => "numbered_step",
    }
}

#[derive(Clone)]
struct AnnotationState {
    tool: Tool,
    font_size: f32,
    stroke_width: f32,
    badge_size: f32,
    color: String,
    opacity: f32,
}

impl Default for AnnotationState {
    fn default() -> Self {
        Self {
            tool: Tool::Text,
            font_size: 24.0,
            stroke_width: 4.0,
            badge_size: 24.0,
            color: "#ff3b30".to_string(),
            opacity: 1.0,
        }
    }
}

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut state = AnnotationState::default();

    for line in stdin.lock().lines() {
        let Ok(line) = line else {
            break;
        };
        let (response, shutdown) = handle_line(&line, &mut state);
        if serde_json::to_writer(&mut stdout, &response).is_err()
            || writeln!(stdout).is_err()
            || stdout.flush().is_err()
        {
            break;
        }
        if shutdown {
            break;
        }
    }
}

fn handle_line(line: &str, state: &mut AnnotationState) -> (JsonRpcResponse, bool) {
    let request: JsonRpcRequest = match serde_json::from_str(line) {
        Ok(request) => request,
        Err(error) => {
            return (
                JsonRpcResponse::error(
                    lumia_plugin_api::RpcId::Number(0),
                    -32700,
                    format!("parse error: {error}"),
                ),
                false,
            );
        }
    };

    let response = match request.method.as_str() {
        "plugin.initialize" => JsonRpcResponse::result(
            request.id,
            json!(InitializeResult {
                protocol_version: PROTOCOL_VERSION,
                manifest: manifest(),
            }),
        ),
        "plugin.capabilities" => JsonRpcResponse::result(
            request.id,
            json!(CapabilitiesResult {
                capabilities: manifest().capabilities,
            }),
        ),
        "ui.activate" => match serde_json::from_value::<UiActivateParams>(request.params) {
            Ok(params) if params.contribution_id == "annotation.panel" => JsonRpcResponse::result(
                request.id,
                json!(UiSessionResult {
                    session_id: SESSION_ID.to_string(),
                    panel: panel_model(state),
                    canvas: Some(canvas_state(state)),
                }),
            ),
            _ => JsonRpcResponse::error(request.id, -32602, "invalid activation parameters"),
        },
        "ui.event" => match serde_json::from_value::<UiEventParams>(request.params) {
            Ok(params) if params.session_id == SESSION_ID => {
                apply_event(state, &params);
                JsonRpcResponse::result(
                    request.id,
                    json!(UiUpdateResult {
                        panel: panel_model(state),
                        canvas: Some(canvas_state(state)),
                    }),
                )
            }
            _ => JsonRpcResponse::error(request.id, -32602, "invalid UI event"),
        },
        "canvas.operation_committed" => {
            match serde_json::from_value::<CanvasOperationCommittedParams>(request.params) {
                Ok(params) if params.session_id == SESSION_ID => {
                    JsonRpcResponse::result(request.id, json!(EmptyResult::default()))
                }
                _ => JsonRpcResponse::error(request.id, -32602, "invalid canvas operation"),
            }
        }
        "ui.close" => JsonRpcResponse::result(request.id, json!(EmptyResult::default())),
        "plugin.shutdown" => {
            return (
                JsonRpcResponse::result(request.id, json!(EmptyResult::default())),
                true,
            );
        }
        method => JsonRpcResponse::error(request.id, -32601, format!("unknown method: {method}")),
    };
    (response, false)
}

fn apply_event(state: &mut AnnotationState, event: &UiEventParams) {
    match (event.control_id.as_str(), &event.value) {
        ("tool", UiValue::String(value)) => {
            state.tool = match value.as_str() {
                "rectangle" => Tool::Rectangle,
                "numbered_step" => Tool::NumberedStep,
                _ => Tool::Text,
            }
        }
        ("font_size", UiValue::Number(value)) => state.font_size = value.clamp(8.0, 256.0),
        ("stroke_width", UiValue::Number(value)) => state.stroke_width = value.clamp(1.0, 64.0),
        ("badge_size", UiValue::Number(value)) => state.badge_size = value.clamp(12.0, 96.0),
        ("color", UiValue::String(value)) if valid_color(value) => state.color.clone_from(value),
        ("opacity", UiValue::Number(value)) => state.opacity = value.clamp(0.1, 1.0),
        _ => {}
    }
}

fn color_control(state: &AnnotationState) -> PanelControl {
    PanelControl::Color {
        id: "color".to_string(),
        label: text("Color", "颜色"),
        value: state.color.clone(),
        enabled: true,
    }
}
fn opacity_control(state: &AnnotationState) -> PanelControl {
    PanelControl::Slider {
        id: "opacity".to_string(),
        label: text("Opacity", "透明度"),
        value: state.opacity,
        min: 0.1,
        max: 1.0,
        step: 0.1,
        enabled: true,
    }
}
fn settings_controls(state: &AnnotationState) -> Vec<PanelControl> {
    match state.tool {
        Tool::Text => vec![
            PanelControl::TextInput {
                id: "text".to_string(),
                label: text("Annotation text", "标注文字"),
                value: String::new(),
                enabled: true,
            },
            PanelControl::Slider {
                id: "font_size".to_string(),
                label: text("Font size", "字号"),
                value: state.font_size,
                min: 8.0,
                max: 256.0,
                step: 2.0,
                enabled: true,
            },
            color_control(state),
            opacity_control(state),
        ],
        Tool::Rectangle => vec![
            PanelControl::Slider {
                id: "stroke_width".to_string(),
                label: text("Stroke width", "线宽"),
                value: state.stroke_width,
                min: 1.0,
                max: 64.0,
                step: 1.0,
                enabled: true,
            },
            color_control(state),
            opacity_control(state),
        ],
        Tool::NumberedStep => vec![
            PanelControl::Slider {
                id: "badge_size".to_string(),
                label: text("Badge size", "徽标大小"),
                value: state.badge_size,
                min: 12.0,
                max: 96.0,
                step: 2.0,
                enabled: true,
            },
            color_control(state),
            opacity_control(state),
        ],
    }
}

fn panel_model(state: &AnnotationState) -> PanelModel {
    PanelModel {
        title: text("Annotation", "标注"),
        sections: vec![
            PanelSection {
                id: "tools".to_string(),
                title: Some(text("Tools", "工具")),
                controls: vec![PanelControl::Select {
                    id: "tool".to_string(),
                    label: text("Tool", "工具"),
                    options: vec![
                        PanelOption {
                            value: "text".to_string(),
                            label: text("Text", "文字"),
                            icon: Some(PluginIcon::Text),
                        },
                        PanelOption {
                            value: "rectangle".to_string(),
                            label: text("Rectangle", "矩形框"),
                            icon: Some(PluginIcon::Rectangle),
                        },
                        PanelOption {
                            value: "numbered_step".to_string(),
                            label: text("Numbered step", "数字步骤"),
                            icon: Some(PluginIcon::NumberedStep),
                        },
                    ],
                    selected: tool_name(state.tool).to_string(),
                    enabled: true,
                }],
            },
            PanelSection {
                id: "tool_settings".to_string(),
                title: Some(text("Settings", "设置")),
                controls: settings_controls(state),
            },
            PanelSection {
                id: "history".to_string(),
                title: None,
                controls: vec![
                    PanelControl::Button {
                        id: "undo".to_string(),
                        label: text("Undo", "撤销"),
                        icon: PluginIcon::Undo,
                        enabled: true,
                    },
                    PanelControl::Button {
                        id: "redo".to_string(),
                        label: text("Redo", "重做"),
                        icon: PluginIcon::Redo,
                        enabled: true,
                    },
                    PanelControl::Button {
                        id: "clear".to_string(),
                        label: text("Clear", "清空"),
                        icon: PluginIcon::Annotation,
                        enabled: true,
                    },
                    PanelControl::Button {
                        id: "export".to_string(),
                        label: text("Export copy", "导出副本"),
                        icon: PluginIcon::Export,
                        enabled: true,
                    },
                ],
            },
        ],
    }
}

fn canvas_state(state: &AnnotationState) -> CanvasToolState {
    let (tool_id, settings) = match state.tool {
        Tool::Text => (
            "annotation.text",
            CanvasToolSettings::Text {
                font_size: state.font_size,
                color: state.color.clone(),
                opacity: state.opacity,
            },
        ),
        Tool::Rectangle => (
            "annotation.rectangle",
            CanvasToolSettings::Rectangle {
                stroke_width: state.stroke_width,
                color: state.color.clone(),
                opacity: state.opacity,
            },
        ),
        Tool::NumberedStep => (
            "annotation.numbered_step",
            CanvasToolSettings::NumberedStep {
                size: state.badge_size,
                color: state.color.clone(),
                opacity: state.opacity,
            },
        ),
    };
    CanvasToolState {
        tool_id: tool_id.to_string(),
        settings,
    }
}

fn manifest() -> PluginManifest {
    PluginManifest {
        id: "lumia.annotation".to_string(),
        name: "Lumia Annotation".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        entry: executable_name().into(),
        capabilities: vec![
            PluginCapability::UiContributions,
            PluginCapability::CanvasOverlay,
        ],
        permissions: Vec::new(),
        supported_inputs: Vec::new(),
        supported_extensions: Vec::new(),
        supported_outputs: vec![
            "image/png".to_string(),
            "image/jpeg".to_string(),
            "image/webp".to_string(),
        ],
        contributions: PluginContributions {
            commands: vec![CommandContribution {
                id: "annotation.open".to_string(),
                label: text("Annotate", "标注"),
                icon: PluginIcon::Annotation,
                requires_document: true,
            }],
            viewer_context_menu: vec![MenuContribution {
                id: "annotation.context_menu".to_string(),
                command_id: "annotation.open".to_string(),
                group: 10,
                order: 10,
            }],
            right_panels: vec![PanelContribution {
                id: "annotation.panel".to_string(),
                command_id: "annotation.open".to_string(),
                title: text("Annotation", "标注"),
            }],
            canvas_tools: vec![
                CanvasToolContribution {
                    id: "annotation.text".to_string(),
                    label: text("Text", "文字"),
                    icon: PluginIcon::Text,
                    kind: CanvasToolKind::Text,
                },
                CanvasToolContribution {
                    id: "annotation.rectangle".to_string(),
                    label: text("Rectangle", "矩形框"),
                    icon: PluginIcon::Rectangle,
                    kind: CanvasToolKind::Rectangle,
                },
                CanvasToolContribution {
                    id: "annotation.numbered_step".to_string(),
                    label: text("Numbered step", "数字步骤"),
                    icon: PluginIcon::NumberedStep,
                    kind: CanvasToolKind::NumberedStep,
                },
            ],
        },
        assets: Vec::new(),
    }
}

fn text(english: &str, chinese: &str) -> LocalizedText {
    LocalizedText {
        fallback: english.to_string(),
        translations: BTreeMap::from([("zh-CN".to_string(), chinese.to_string())]),
    }
}

fn valid_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn executable_name() -> &'static str {
    if cfg!(windows) {
        "lumia-plugin-annotation.exe"
    } else {
        "lumia-plugin-annotation"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(control_id: &str, value: UiValue) -> UiEventParams {
        UiEventParams {
            session_id: SESSION_ID.to_string(),
            control_id: control_id.to_string(),
            value,
        }
    }

    fn find_slider<'a>(panel: &'a PanelModel, id: &str) -> Option<&'a PanelControl> {
        panel
            .sections
            .iter()
            .flat_map(|section| section.controls.iter())
            .find(|control| control.id() == id)
    }

    #[test]
    fn panel_events_update_bounded_marker_state() {
        let mut state = AnnotationState::default();
        apply_event(&mut state, &event("font_size", UiValue::Number(500.0)));
        assert_eq!(state.font_size, 256.0);
        apply_event(&mut state, &event("font_size", UiValue::Number(12.0)));
        assert_eq!(state.font_size, 12.0);
        apply_event(&mut state, &event("opacity", UiValue::Number(2.0)));
        assert_eq!(state.opacity, 1.0);

        let panel = panel_model(&state);
        assert!(matches!(
            find_slider(&panel, "font_size"),
            Some(PanelControl::Slider { value: 12.0, .. })
        ));
    }
    #[test]
    fn tool_select_switches_canvas_settings_variant() {
        let mut state = AnnotationState::default();
        apply_event(&mut state, &event("tool", UiValue::String("rectangle".to_string())));
        assert!(matches!(
            canvas_state(&state).settings,
            CanvasToolSettings::Rectangle { .. }
        ));
        assert_eq!(canvas_state(&state).tool_id, "annotation.rectangle");

        apply_event(&mut state, &event("tool", UiValue::String("numbered_step".to_string())));
        assert!(matches!(
            canvas_state(&state).settings,
            CanvasToolSettings::NumberedStep { size: 24.0, .. }
        ));
        assert_eq!(canvas_state(&state).tool_id, "annotation.numbered_step");
    }
    #[test]
    fn panel_controls_adapt_to_the_selected_tool() {
        let mut state = AnnotationState::default();
        apply_event(&mut state, &event("tool", UiValue::String("rectangle".to_string())));
        let panel = panel_model(&state);
        assert!(find_slider(&panel, "stroke_width").is_some());
        assert!(find_slider(&panel, "font_size").is_none());
        assert!(find_slider(&panel, "badge_size").is_none());

        apply_event(&mut state, &event("tool", UiValue::String("numbered_step".to_string())));
        let panel = panel_model(&state);
        assert!(find_slider(&panel, "badge_size").is_some());
        assert!(find_slider(&panel, "stroke_width").is_none());
    }
    #[test]
    fn manifest_declares_three_canvas_tools_and_no_assets() {
        let manifest = manifest();
        assert!(manifest
            .capabilities
            .contains(&PluginCapability::UiContributions));
        assert!(manifest
            .capabilities
            .contains(&PluginCapability::CanvasOverlay));
        assert_eq!(manifest.contributions.canvas_tools.len(), 3);
        assert!(manifest.assets.is_empty());
    }
}
