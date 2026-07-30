use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};

use lumia_plugin_api::{
    CanvasOperationCommittedParams, CanvasToolContribution, CanvasToolKind, CanvasToolSettings,
    CanvasToolState, CapabilitiesResult, CommandContribution, EmptyResult, InitializeResult,
    JsonRpcRequest, JsonRpcResponse, LocalizedText, MenuContribution, PanelContribution,
    PanelControl, PanelModel, PanelOption, PanelSection, PluginAsset, PluginCapability,
    PluginContributions, PluginIcon, PluginManifest, UiActivateParams, UiEventParams,
    UiSessionResult, UiUpdateResult, UiValue, PROTOCOL_VERSION,
};
use serde_json::json;

const SESSION_ID: &str = "annotation-session";

#[derive(Clone)]
struct AnnotationState {
    asset_id: String,
    size: f32,
    color: String,
    opacity: f32,
}

impl Default for AnnotationState {
    fn default() -> Self {
        Self {
            asset_id: "pin".to_string(),
            size: 48.0,
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
        ("icon", UiValue::String(value)) if matches!(value.as_str(), "pin" | "star" | "check") => {
            state.asset_id.clone_from(value)
        }
        ("size", UiValue::Number(value)) => state.size = value.clamp(16.0, 128.0),
        ("color", UiValue::String(value)) if valid_color(value) => state.color.clone_from(value),
        ("opacity", UiValue::Number(value)) => state.opacity = value.clamp(0.1, 1.0),
        _ => {}
    }
}

fn panel_model(state: &AnnotationState) -> PanelModel {
    PanelModel {
        title: text("Annotation", "标注"),
        sections: vec![
            PanelSection {
                id: "marker".to_string(),
                title: Some(text("Icon marker", "图标标记")),
                controls: vec![
                    PanelControl::Select {
                        id: "icon".to_string(),
                        label: text("Icon", "图标"),
                        options: ["pin", "star", "check"]
                            .into_iter()
                            .map(|value| PanelOption {
                                value: value.to_string(),
                                label: text(value, value),
                                icon: Some(PluginIcon::Asset(value.to_string())),
                            })
                            .collect(),
                        selected: state.asset_id.clone(),
                        enabled: true,
                    },
                    PanelControl::Slider {
                        id: "size".to_string(),
                        label: text("Size", "大小"),
                        value: state.size,
                        min: 16.0,
                        max: 128.0,
                        step: 4.0,
                        enabled: true,
                    },
                    PanelControl::Color {
                        id: "color".to_string(),
                        label: text("Color", "颜色"),
                        value: state.color.clone(),
                        enabled: true,
                    },
                    PanelControl::Slider {
                        id: "opacity".to_string(),
                        label: text("Opacity", "透明度"),
                        value: state.opacity,
                        min: 0.1,
                        max: 1.0,
                        step: 0.1,
                        enabled: true,
                    },
                ],
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
    CanvasToolState {
        tool_id: "annotation.icon_stamp".to_string(),
        settings: CanvasToolSettings::IconStamp {
            asset_id: state.asset_id.clone(),
            size: state.size,
            color: state.color.clone(),
            opacity: state.opacity,
        },
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
            canvas_tools: vec![CanvasToolContribution {
                id: "annotation.icon_stamp".to_string(),
                label: text("Icon marker", "图标标记"),
                icon: PluginIcon::Annotation,
                kind: CanvasToolKind::IconStamp,
            }],
        },
        assets: ["pin", "star", "check"]
            .into_iter()
            .map(|id| PluginAsset {
                id: id.to_string(),
                path: format!("assets/{id}.svg").into(),
                media_type: "image/svg+xml".to_string(),
                sha256: String::new(),
            })
            .collect(),
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

    #[test]
    fn panel_events_update_bounded_marker_state() {
        let mut state = AnnotationState::default();
        apply_event(
            &mut state,
            &UiEventParams {
                session_id: SESSION_ID.to_string(),
                control_id: "size".to_string(),
                value: UiValue::Number(500.0),
            },
        );
        assert_eq!(state.size, 128.0);
        assert!(matches!(
            &panel_model(&state).sections[0].controls[1],
            PanelControl::Slider { value: 128.0, .. }
        ));
    }

    #[test]
    fn manifest_declares_ui_and_canvas_capabilities() {
        let manifest = manifest();
        assert!(manifest
            .capabilities
            .contains(&PluginCapability::UiContributions));
        assert!(manifest
            .capabilities
            .contains(&PluginCapability::CanvasOverlay));
    }
}
