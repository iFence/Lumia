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
    #[error("plugin returned rpc error {code}: {message}")]
    Rpc { code: i64, message: String },
}

pub type Result<T> = std::result::Result<T, PluginHostError>;
