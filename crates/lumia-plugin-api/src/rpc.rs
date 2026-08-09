use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const JSON_RPC_VERSION: &str = "2.0";
pub const PROTOCOL_VERSION: u32 = 3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RpcId {
    Number(u64),
    String(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: RpcId,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

impl JsonRpcRequest {
    pub fn new(id: RpcId, method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id,
            method: method.into(),
            params,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: RpcId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl JsonRpcResponse {
    pub fn result(id: RpcId, value: Value) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id,
            result: Some(value),
            error: None,
        }
    }

    pub fn error(id: RpcId, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id,
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }

    pub fn plugin_error(
        id: RpcId,
        code: i64,
        message: impl Into<String>,
        kind: PluginErrorKind,
    ) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION.to_string(),
            id,
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
                data: Some(serde_json::json!(PluginErrorData { kind })),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcError {
    pub fn plugin_kind(&self) -> Option<PluginErrorKind> {
        self.data
            .as_ref()
            .and_then(|data| serde_json::from_value::<PluginErrorData>(data.clone()).ok())
            .map(|data| data.kind)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginErrorKind {
    UnsupportedFormat,
    CorruptImage,
    ResourceLimit,
    DecodeFailed,
    Cancelled,
    PluginUnavailable,
    ProtocolMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginErrorData {
    pub kind: PluginErrorKind,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_error_serializes_stable_category() {
        let response = JsonRpcResponse::plugin_error(
            RpcId::Number(7),
            -32010,
            "image exceeds safety limits",
            PluginErrorKind::ResourceLimit,
        );

        let value = serde_json::to_value(response).unwrap();
        assert_eq!(value["error"]["data"]["kind"], "resource_limit");
    }
}
