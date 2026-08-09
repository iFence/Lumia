use std::io::{self, BufRead, Write};

use lumia_plugin_api::{
    CanvasOperationCommittedParams, CanvasToolContribution, CanvasToolKind, CanvasToolSettings,
    CanvasToolState, CapabilitiesResult, CommandContribution, EmptyResult, InitializeResult,
    JsonRpcRequest, JsonRpcResponse, MenuContribution, PanelContribution, PluginCapability,
    PluginContributions, PluginIcon, PluginManifest, UiActivateParams, UiEventParams,
    UiSessionResult, UiUpdateResult, UiValue, PROTOCOL_VERSION,
};
use serde_json::json;

const SESSION_ID: &str = "annotation-session";

mod panel;
mod sizing;

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
            Ok(params) if params.contribution_id == "annotation.panel" => {
                apply_image_dimension_defaults(
                    state,
                    params.document.width,
                    params.document.height,
                );
                JsonRpcResponse::result(
                    request.id,
                    json!(UiSessionResult {
                        session_id: SESSION_ID.to_string(),
                        panel: panel::panel_model(state),
                        canvas: Some(canvas_state(state)),
                    }),
                )
            }
            _ => JsonRpcResponse::error(request.id, -32602, "invalid activation parameters"),
        },
        "ui.event" => match serde_json::from_value::<UiEventParams>(request.params) {
            Ok(params) if params.session_id == SESSION_ID => {
                apply_event(state, &params);
                JsonRpcResponse::result(
                    request.id,
                    json!(UiUpdateResult {
                        panel: panel::panel_model(state),
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

fn apply_image_dimension_defaults(state: &mut AnnotationState, width: u32, height: u32) {
    let defaults = sizing::defaults_for_image(width, height);
    state.font_size = defaults.font_size;
    state.stroke_width = defaults.stroke_width;
    state.badge_size = defaults.badge_size;
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
                label: panel::text("Annotate", "标注"),
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
                title: panel::text("Annotation", "标注"),
            }],
            canvas_tools: vec![
                CanvasToolContribution {
                    id: "annotation.text".to_string(),
                    label: panel::text("Text", "文字"),
                    icon: PluginIcon::Text,
                    kind: CanvasToolKind::Text,
                },
                CanvasToolContribution {
                    id: "annotation.rectangle".to_string(),
                    label: panel::text("Rectangle", "矩形框"),
                    icon: PluginIcon::Rectangle,
                    kind: CanvasToolKind::Rectangle,
                },
                CanvasToolContribution {
                    id: "annotation.numbered_step".to_string(),
                    label: panel::text("Numbered step", "数字步骤"),
                    icon: PluginIcon::NumberedStep,
                    kind: CanvasToolKind::NumberedStep,
                },
            ],
        },
        assets: Vec::new(),
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
    use lumia_plugin_api::{PanelControl, PanelModel, RpcId};

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

        let panel = panel::panel_model(&state);
        assert!(matches!(
            find_slider(&panel, "font_size"),
            Some(PanelControl::Slider { value: 12.0, .. })
        ));
    }
    #[test]
    fn tool_select_switches_canvas_settings_variant() {
        let mut state = AnnotationState::default();
        apply_event(
            &mut state,
            &event("tool", UiValue::String("rectangle".to_string())),
        );
        assert!(matches!(
            canvas_state(&state).settings,
            CanvasToolSettings::Rectangle { .. }
        ));
        assert_eq!(canvas_state(&state).tool_id, "annotation.rectangle");

        apply_event(
            &mut state,
            &event("tool", UiValue::String("numbered_step".to_string())),
        );
        assert!(matches!(
            canvas_state(&state).settings,
            CanvasToolSettings::NumberedStep { size: 24.0, .. }
        ));
        assert_eq!(canvas_state(&state).tool_id, "annotation.numbered_step");
    }
    #[test]
    fn panel_controls_adapt_to_the_selected_tool() {
        let mut state = AnnotationState::default();
        apply_event(
            &mut state,
            &event("tool", UiValue::String("rectangle".to_string())),
        );
        let panel = panel::panel_model(&state);
        assert!(find_slider(&panel, "stroke_width").is_some());
        assert!(find_slider(&panel, "font_size").is_none());
        assert!(find_slider(&panel, "badge_size").is_none());

        apply_event(
            &mut state,
            &event("tool", UiValue::String("numbered_step".to_string())),
        );
        let panel = panel::panel_model(&state);
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

    fn activate_line(width: u32, height: u32) -> String {
        serde_json::to_string(&JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: RpcId::Number(1),
            method: "ui.activate".to_string(),
            params: json!({
                "contribution_id": "annotation.panel",
                "document": {
                    "document_id": "doc",
                    "width": width,
                    "height": height,
                    "rotation_quarter_turns": 0,
                },
            }),
        })
        .unwrap()
    }

    #[test]
    fn ui_activate_applies_image_sized_defaults() {
        let mut state = AnnotationState::default();
        let (response, _) = handle_line(&activate_line(8000, 6000), &mut state);

        let session: UiSessionResult =
            serde_json::from_value(response.result.expect("activation should succeed")).unwrap();
        assert!(matches!(
            session.canvas.expect("canvas should be present").settings,
            CanvasToolSettings::Text {
                font_size: 96.0,
                ..
            }
        ));
        assert_eq!(state.stroke_width, 16.0);
        assert_eq!(state.badge_size, 80.0);

        let panel = panel::panel_model(&state);
        assert!(matches!(
            find_slider(&panel, "font_size"),
            Some(PanelControl::Slider { value: 96.0, .. })
        ));
    }

    #[test]
    fn ui_activate_rejects_unknown_contribution() {
        let mut state = AnnotationState::default();
        let line = serde_json::to_string(&JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: RpcId::Number(1),
            method: "ui.activate".to_string(),
            params: json!({
                "contribution_id": "other.panel",
                "document": {
                    "document_id": "doc",
                    "width": 8000,
                    "height": 6000,
                    "rotation_quarter_turns": 0,
                },
            }),
        })
        .unwrap();
        let (response, _) = handle_line(&line, &mut state);

        assert_eq!(
            response.error.map(|error| error.code),
            Some(-32602),
            "unknown panel should be rejected"
        );
        assert_eq!(state.font_size, 24.0);
        assert_eq!(state.stroke_width, 4.0);
        assert_eq!(state.badge_size, 24.0);
    }

    #[test]
    fn user_slider_adjustment_overrides_adaptive_defaults() {
        let mut state = AnnotationState::default();
        handle_line(&activate_line(8000, 6000), &mut state);
        assert_eq!(state.font_size, 96.0);

        apply_event(&mut state, &event("font_size", UiValue::Number(40.0)));
        assert_eq!(state.font_size, 40.0);
    }
}
