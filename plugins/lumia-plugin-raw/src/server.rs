use std::io::{self, BufRead, Write};

use lumia_plugin_api::{
    CapabilitiesResult, DecodePreviewParams, InitializeResult, JsonRpcRequest, JsonRpcResponse,
    PluginCapability, PluginManifest, PluginPermission, ProbeParams, PROTOCOL_VERSION,
};
use serde_json::json;

use crate::decode::{decode_preview, probe, RawError, RAW_EXTENSIONS};

pub(crate) fn run() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let (response, shutdown) = handle_line(&line);
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

fn handle_line(line: &str) -> (JsonRpcResponse, bool) {
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
        "image.probe" => match serde_json::from_value::<ProbeParams>(request.params) {
            Ok(params) => match probe(&params.input.path) {
                Ok(result) => JsonRpcResponse::result(request.id, json!(result)),
                Err(error) => plugin_error(request.id, error, -32020),
            },
            Err(_) => JsonRpcResponse::error(request.id, -32602, "invalid probe parameters"),
        },
        "image.decode_preview" => {
            match serde_json::from_value::<DecodePreviewParams>(request.params) {
                Ok(params) => match decode_preview(params) {
                    Ok(result) => JsonRpcResponse::result(request.id, json!(result)),
                    Err(error) => plugin_error(request.id, error, -32021),
                },
                Err(_) => JsonRpcResponse::error(request.id, -32602, "invalid decode parameters"),
            }
        }
        "plugin.shutdown" => {
            return (
                JsonRpcResponse::result(
                    request.id,
                    json!(lumia_plugin_api::EmptyResult::default()),
                ),
                true,
            );
        }
        method => JsonRpcResponse::error(request.id, -32601, format!("unknown method: {method}")),
    };
    (response, false)
}

fn plugin_error(id: lumia_plugin_api::RpcId, error: RawError, code: i64) -> JsonRpcResponse {
    JsonRpcResponse::plugin_error(id, code, error.to_string(), error.kind())
}

pub(crate) fn manifest() -> PluginManifest {
    PluginManifest {
        id: "lumia.raw".to_string(),
        name: "Lumia RAW Preview".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        entry: executable_name().into(),
        capabilities: vec![PluginCapability::Probe, PluginCapability::DecodePreview],
        permissions: vec![
            PluginPermission::ReadInputPath,
            PluginPermission::WriteTemporaryOutput,
        ],
        supported_inputs: vec!["image/x-camera-raw".to_string()],
        supported_extensions: RAW_EXTENSIONS
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        supported_outputs: vec!["image/png".to_string()],
        contributions: Default::default(),
        assets: Vec::new(),
    }
}

fn executable_name() -> &'static str {
    if cfg!(windows) {
        "lumia-plugin-raw.exe"
    } else {
        "lumia-plugin-raw"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_declares_all_raw_extensions_and_decoder_capabilities() {
        let manifest = manifest();
        assert_eq!(manifest.id, "lumia.raw");
        assert_eq!(manifest.supported_extensions.len(), 25);
        assert!(manifest.capabilities.contains(&PluginCapability::Probe));
        assert!(manifest
            .capabilities
            .contains(&PluginCapability::DecodePreview));
    }
}
