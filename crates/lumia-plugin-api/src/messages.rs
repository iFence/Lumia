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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProbeResult {
    pub can_decode: bool,
    pub format_name: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub is_hdr: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<PluginImageMetadata>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PluginImageMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera_make: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lens: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iso: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exposure_time_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aperture_f_number: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focal_length_mm: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_taken: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geo_coordinates: Option<PluginGeoCoordinates>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PluginGeoCoordinates {
    pub latitude: f64,
    pub longitude: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub altitude_meters: Option<f64>,
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
    TextInput {
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
            | Self::Text { id, .. }
            | Self::TextInput { id, .. } => id,
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
    Text {
        font_size: f32,
        color: String,
        opacity: f32,
    },
    Rectangle {
        stroke_width: f32,
        color: String,
        opacity: f32,
    },
    NumberedStep {
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
    TextPlaced {
        text: String,
        x: f32,
        y: f32,
        font_size: f32,
        color: String,
        opacity: f32,
    },
    RectanglePlaced {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        stroke_width: f32,
        color: String,
        opacity: f32,
    },
    StepPlaced {
        number: u32,
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
            supported_extensions: vec!["png".to_string()],
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
            tool_id: "annotation.text".to_string(),
            settings: CanvasToolSettings::Text {
                font_size: 24.0,
                color: "#ff3b30".to_string(),
                opacity: 1.0,
            },
        };
        let value = serde_json::to_value(state).unwrap();
        assert_eq!(value["tool_id"], "annotation.text");
        assert_eq!(value["settings"]["type"], "text");
        assert_eq!(value["settings"]["font_size"], 24.0);
    }

    #[test]
    fn old_manifest_defaults_supported_extensions() {
        let manifest: PluginManifest = serde_json::from_value(serde_json::json!({
            "id": "legacy.decoder",
            "name": "Legacy decoder",
            "version": "1.0.0",
            "entry": "legacy-decoder",
            "capabilities": ["probe", "decode_preview"],
            "permissions": ["read_input_path", "write_temporary_output"],
            "supported_inputs": ["image/example"],
            "supported_outputs": ["image/png"]
        }))
        .unwrap();
        assert!(manifest.supported_extensions.is_empty());
    }

    #[test]
    fn probe_metadata_uses_structured_stable_fields() {
        let result = ProbeResult {
            can_decode: true,
            format_name: Some("DNG".into()),
            width: Some(6000),
            height: Some(4000),
            is_hdr: false,
            metadata: Some(PluginImageMetadata {
                camera_make: Some("Example".into()),
                camera_model: Some("Camera".into()),
                lens: Some("50mm".into()),
                iso: Some(200),
                exposure_time_seconds: Some(0.008),
                aperture_f_number: Some(2.8),
                focal_length_mm: Some(50.0),
                date_taken: Some("2026-01-02T03:04:05".into()),
                geo_coordinates: Some(PluginGeoCoordinates {
                    latitude: 31.2304,
                    longitude: 121.4737,
                    altitude_meters: Some(4.0),
                }),
            }),
        };

        let value = serde_json::to_value(result).unwrap();
        assert_eq!(value["metadata"]["iso"], 200);
        assert_eq!(value["metadata"]["geo_coordinates"]["latitude"], 31.2304);
        assert_eq!(value["metadata"]["aperture_f_number"], 2.8);
    }
}
