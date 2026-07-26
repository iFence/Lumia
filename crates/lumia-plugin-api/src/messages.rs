use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::manifest::{LocalizedText, PluginCapability, PluginIcon, PluginManifest};

use super::rpc::PROTOCOL_VERSION;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitializeParams {
    pub protocol_version: u32,
    pub host_name: String,
    pub host_version: String,
}

impl Default for InitializeParams {
    fn default() -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            host_name: "lumia".to_string(),
            host_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitializeResult {
    pub protocol_version: u32,
    pub manifest: PluginManifest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilitiesResult {
    pub capabilities: Vec<PluginCapability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImagePath {
    pub path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeParams {
    pub input: ImagePath,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbeResult {
    pub can_decode: bool,
    pub format_name: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub is_hdr: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecodePreviewParams {
    pub input: ImagePath,
    pub output_path: PathBuf,
    pub max_width: u32,
    pub max_height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecodePreviewResult {
    pub output: ImageOutput,
    pub width: u32,
    pub height: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransformParams {
    pub input: ImagePath,
    pub output_path: PathBuf,
    pub operations: Vec<ImageOperation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageOperation {
    Convert {
        format: String,
    },
    Compress {
        quality: u8,
    },
    Crop {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
    SuperResolution {
        scale: u8,
    },
    CloudAi {
        provider: String,
        prompt: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageOutput {
    pub path: PathBuf,
    pub media_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskCancelParams {
    pub task_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentContext {
    pub document_id: String,
    pub width: u32,
    pub height: u32,
    pub rotation_quarter_turns: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiActivateParams {
    pub contribution_id: String,
    pub document: DocumentContext,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiSessionResult {
    pub session_id: String,
    pub panel: PanelModel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canvas: Option<CanvasToolState>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PanelModel {
    pub title: LocalizedText,
    #[serde(default)]
    pub sections: Vec<PanelSection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PanelSection {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<LocalizedText>,
    #[serde(default)]
    pub controls: Vec<PanelControl>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PanelControl {
    Button {
        id: String,
        label: LocalizedText,
        icon: PluginIcon,
        enabled: bool,
    },
    Toggle {
        id: String,
        label: LocalizedText,
        value: bool,
        enabled: bool,
    },
    Select {
        id: String,
        label: LocalizedText,
        options: Vec<PanelOption>,
        selected: String,
        enabled: bool,
    },
    Slider {
        id: String,
        label: LocalizedText,
        value: f32,
        min: f32,
        max: f32,
        step: f32,
        enabled: bool,
    },
    Color {
        id: String,
        label: LocalizedText,
        value: String,
        enabled: bool,
    },
    Text {
        id: String,
        label: LocalizedText,
        value: String,
        enabled: bool,
    },
}

impl PanelControl {
    pub fn id(&self) -> &str {
        match self {
            Self::Button { id, .. }
            | Self::Toggle { id, .. }
            | Self::Select { id, .. }
            | Self::Slider { id, .. }
            | Self::Color { id, .. }
            | Self::Text { id, .. } => id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelOption {
    pub value: String,
    pub label: LocalizedText,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<PluginIcon>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiEventParams {
    pub session_id: String,
    pub control_id: String,
    pub value: UiValue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum UiValue {
    Bool(bool),
    Number(f32),
    String(String),
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UiUpdateResult {
    pub panel: PanelModel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canvas: Option<CanvasToolState>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasToolState {
    pub tool_id: String,
    pub settings: CanvasToolSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CanvasToolSettings {
    IconStamp {
        asset_id: String,
        size: f32,
        color: String,
        opacity: f32,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasOperationCommittedParams {
    pub session_id: String,
    pub operation: CanvasOperation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CanvasOperation {
    IconPlaced {
        asset_id: String,
        x: f32,
        y: f32,
        size: f32,
        color: String,
        opacity: f32,
    },
    Cleared,
    Undo,
    Redo,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiCloseParams {
    pub session_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EmptyResult {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_uses_snake_case_capabilities() {
        let manifest = PluginManifest {
            id: "sample".to_string(),
            name: "Sample".to_string(),
            version: "0.1.0".to_string(),
            entry: "sample".into(),
            capabilities: vec![PluginCapability::DecodePreview, PluginCapability::CloudAi],
            permissions: vec![super::super::manifest::PluginPermission::Network],
            supported_inputs: vec!["image/png".to_string()],
            supported_outputs: vec!["image/png".to_string()],
            contributions: super::super::manifest::PluginContributions::default(),
            assets: Vec::new(),
        };

        let json = serde_json::to_string(&manifest).unwrap();
        assert!(json.contains("decode_preview"));
        assert!(json.contains("cloud_ai"));
    }

    #[test]
    fn decode_preview_result_serializes_output_metadata() {
        let result = DecodePreviewResult {
            output: ImageOutput {
                path: "preview.png".into(),
                media_type: Some("image/png".to_string()),
            },
            width: 640,
            height: 480,
            format_name: Some("PSD".to_string()),
        };

        let value = serde_json::to_value(result).unwrap();
        assert_eq!(value["output"]["path"], "preview.png");
        assert_eq!(value["width"], 640);
        assert_eq!(value["height"], 480);
        assert_eq!(value["format_name"], "PSD");
    }

    #[test]
    fn panel_controls_use_stable_tagged_shapes() {
        let control = PanelControl::Toggle {
            id: "enabled".to_string(),
            label: LocalizedText {
                fallback: "Enabled".to_string(),
                translations: Default::default(),
            },
            value: true,
            enabled: true,
        };

        let value = serde_json::to_value(control).unwrap();
        assert_eq!(value["type"], "toggle");
        assert_eq!(value["id"], "enabled");
        assert_eq!(value["value"], true);
    }

    #[test]
    fn canvas_tool_state_uses_a_typed_settings_shape() {
        let state = CanvasToolState {
            tool_id: "annotation.icon_stamp".to_string(),
            settings: CanvasToolSettings::IconStamp {
                asset_id: "pin".to_string(),
                size: 48.0,
                color: "#ff3b30".to_string(),
                opacity: 1.0,
            },
        };
        let value = serde_json::to_value(state).unwrap();
        assert_eq!(value["tool_id"], "annotation.icon_stamp");
        assert_eq!(value["settings"]["type"], "icon_stamp");
        assert_eq!(value["settings"]["asset_id"], "pin");
    }
}
