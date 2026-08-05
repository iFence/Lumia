use std::io::{self, BufRead, Write};

use lumia_plugin_api::{
    CapabilitiesResult, InitializeResult, JsonRpcRequest, JsonRpcResponse, PluginCapability,
    PluginManifest, PluginPermission, ProbeParams, PROTOCOL_VERSION,
};
use serde_json::json;

use crate::decode::{decode_preview, DecodeError};
use crate::probe::{probe, ProbeError};

pub(crate) fn run() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let response = match line {
            Ok(line) => handle_line(&line),
            Err(_) => break,
        };
        if serde_json::to_writer(&mut stdout, &response).is_err()
            || writeln!(stdout).is_err()
            || stdout.flush().is_err()
        {
            break;
        }
    }
}

fn handle_line(line: &str) -> JsonRpcResponse {
    let request: JsonRpcRequest = match serde_json::from_str(line) {
        Ok(request) => request,
        Err(error) => {
            return JsonRpcResponse::error(
                lumia_plugin_api::RpcId::Number(0),
                -32700,
                format!("parse error: {error}"),
            );
        }
    };
    match request.method.as_str() {
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
                Err(error) => probe_error(request.id, error),
            },
            Err(_) => JsonRpcResponse::error(request.id, -32602, "invalid probe parameters"),
        },
        "image.decode_preview" => {
            match serde_json::from_value::<lumia_plugin_api::DecodePreviewParams>(request.params) {
                Ok(params) => match decode_preview(params) {
                    Ok(result) => JsonRpcResponse::result(request.id, json!(result)),
                    Err(error) => decode_error(request.id, error),
                },
                Err(_) => JsonRpcResponse::error(request.id, -32602, "invalid decode parameters"),
            }
        }
        "plugin.shutdown" => {
            JsonRpcResponse::result(request.id, json!(lumia_plugin_api::EmptyResult::default()))
        }
        method => JsonRpcResponse::error(request.id, -32601, format!("unknown method: {method}")),
    }
}

fn decode_error(id: lumia_plugin_api::RpcId, error: DecodeError) -> JsonRpcResponse {
    JsonRpcResponse::plugin_error(id, -32021, error.to_string(), error.kind())
}

fn probe_error(id: lumia_plugin_api::RpcId, error: ProbeError) -> JsonRpcResponse {
    JsonRpcResponse::plugin_error(
        id,
        -32020,
        error.to_string(),
        lumia_plugin_api::PluginErrorKind::CorruptImage,
    )
}

pub(crate) fn manifest() -> PluginManifest {
    PluginManifest {
        id: "lumia.jpeg-xl".to_string(),
        name: "Lumia JPEG XL Preview".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        entry: executable_name().into(),
        capabilities: vec![PluginCapability::Probe, PluginCapability::DecodePreview],
        permissions: vec![
            PluginPermission::ReadInputPath,
            PluginPermission::WriteTemporaryOutput,
        ],
        supported_inputs: vec!["image/jxl".to_string()],
        supported_extensions: vec!["jxl".to_string()],
        supported_outputs: vec!["image/png".to_string()],
        contributions: Default::default(),
        assets: Vec::new(),
    }
}

fn executable_name() -> &'static str {
    if cfg!(windows) {
        "lumia-plugin-jpeg-xl.exe"
    } else {
        "lumia-plugin-jpeg-xl"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_declares_jpeg_xl_preview_contract() {
        let manifest = manifest();
        assert_eq!(manifest.id, "lumia.jpeg-xl");
        assert_eq!(manifest.supported_extensions, ["jxl"]);
        assert_eq!(manifest.supported_inputs, ["image/jxl"]);
        assert!(manifest.capabilities.contains(&PluginCapability::Probe));
        assert!(manifest
            .capabilities
            .contains(&PluginCapability::DecodePreview));
    }
}
