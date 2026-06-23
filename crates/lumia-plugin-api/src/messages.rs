use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::manifest::{PluginCapability, PluginManifest};

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
        };

        let json = serde_json::to_string(&manifest).unwrap();
        assert!(json.contains("decode_preview"));
        assert!(json.contains("cloud_ai"));
    }
}
