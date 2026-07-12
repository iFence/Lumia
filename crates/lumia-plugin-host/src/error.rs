use lumia_plugin_api::{PluginCapability, PluginErrorKind, PluginPermission};

#[derive(Debug, thiserror::Error)]
pub enum PluginHostError {
    #[error("plugin entry is empty")]
    EmptyEntry,
    #[error("failed to spawn plugin: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("plugin stdin is unavailable")]
    MissingStdin,
    #[error("plugin stdout is unavailable")]
    MissingStdout,
    #[error("failed to serialize json-rpc message: {0}")]
    Serialize(#[source] serde_json::Error),
    #[error("failed to deserialize json-rpc message: {0}")]
    Deserialize(#[source] serde_json::Error),
    #[error("plugin closed stdout")]
    Closed,
    #[error("plugin returned json-rpc version {actual}, expected {expected}")]
    JsonRpcVersion {
        expected: &'static str,
        actual: String,
    },
    #[error("plugin response id does not match request id")]
    ResponseId,
    #[error("plugin protocol version {actual} does not match host version {expected}")]
    ProtocolMismatch { expected: u32, actual: u32 },
    #[error("plugin id {actual} does not match expected id {expected}")]
    PluginIdMismatch { expected: String, actual: String },
    #[error("plugin does not declare required capability {0:?}")]
    MissingCapability(PluginCapability),
    #[error("plugin does not declare required permission {0:?}")]
    MissingPermission(PluginPermission),
    #[error("plugin returned rpc error {code}: {message}")]
    Rpc {
        code: i64,
        message: String,
        kind: Option<PluginErrorKind>,
    },
}

pub type Result<T> = std::result::Result<T, PluginHostError>;
