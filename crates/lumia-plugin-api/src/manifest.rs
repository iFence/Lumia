use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub entry: PathBuf,
    #[serde(default)]
    pub capabilities: Vec<PluginCapability>,
    #[serde(default)]
    pub permissions: Vec<PluginPermission>,
    #[serde(default)]
    pub supported_inputs: Vec<String>,
    /// Lowercase file extensions without a leading dot.
    ///
    /// This is optional on the wire so protocol-v2 manifests created before
    /// extension-based decoder dispatch remain readable.
    #[serde(default)]
    pub supported_extensions: Vec<String>,
    #[serde(default)]
    pub supported_outputs: Vec<String>,
    #[serde(default)]
    pub contributions: PluginContributions,
    #[serde(default)]
    pub assets: Vec<PluginAsset>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginCapability {
    Probe,
    DecodePreview,
    Transform,
    ConvertFormat,
    Compress,
    Crop,
    SuperResolution,
    CloudAi,
    UiContributions,
    CanvasOverlay,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginPermission {
    ReadInputPath,
    WriteTemporaryOutput,
    Network,
    ReadConfigSecret(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PluginContributions {
    #[serde(default)]
    pub commands: Vec<CommandContribution>,
    #[serde(default)]
    pub viewer_context_menu: Vec<MenuContribution>,
    #[serde(default)]
    pub right_panels: Vec<PanelContribution>,
    #[serde(default)]
    pub canvas_tools: Vec<CanvasToolContribution>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandContribution {
    pub id: String,
    pub label: LocalizedText,
    pub icon: PluginIcon,
    #[serde(default)]
    pub requires_document: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuContribution {
    pub id: String,
    pub command_id: String,
    #[serde(default)]
    pub group: u16,
    #[serde(default)]
    pub order: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelContribution {
    pub id: String,
    pub command_id: String,
    pub title: LocalizedText,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CanvasToolContribution {
    pub id: String,
    pub label: LocalizedText,
    pub icon: PluginIcon,
    pub kind: CanvasToolKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CanvasToolKind {
    Select,
    Text,
    Rectangle,
    Ellipse,
    Arrow,
    NumberedStep,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum PluginIcon {
    Annotation,
    Select,
    Text,
    Rectangle,
    Ellipse,
    Arrow,
    NumberedStep,
    Undo,
    Redo,
    Export,
    Asset(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginAsset {
    pub id: String,
    pub path: PathBuf,
    pub media_type: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalizedText {
    pub fallback: String,
    #[serde(default)]
    pub translations: BTreeMap<String, String>,
}

impl LocalizedText {
    pub fn resolve(&self, language: &str) -> &str {
        self.translations
            .get(language)
            .map(String::as_str)
            .unwrap_or(&self.fallback)
    }
}
