use lumia_plugin_api::{
    CapabilitiesResult, InitializeResult, JsonRpcRequest, JsonRpcResponse, PluginCapability,
    PluginManifest, PluginPermission, PROTOCOL_VERSION,
};
use serde_json::json;
use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let response = match line {
            Ok(line) => handle_line(&line),
            Err(error) => {
                let _ = writeln!(io::stderr(), "failed to read request: {error}");
                break;
            }
        };

        if let Ok(serialized) = serde_json::to_string(&response) {
            let _ = writeln!(stdout, "{serialized}");
            let _ = stdout.flush();
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
        "image.probe" => JsonRpcResponse::result(
            request.id,
            json!({
                "can_decode": false,
                "format_name": null,
                "width": null,
                "height": null,
                "is_hdr": false
            }),
        ),
        method => JsonRpcResponse::error(request.id, -32601, format!("unknown method: {method}")),
    }
}

fn manifest() -> PluginManifest {
    PluginManifest {
        id: "lumia.sample".to_string(),
        name: "Lumia Sample Plugin".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        entry: "lumia-plugin-sample".into(),
        capabilities: vec![PluginCapability::Probe],
        permissions: vec![
            PluginPermission::ReadInputPath,
            PluginPermission::WriteTemporaryOutput,
        ],
        supported_inputs: vec!["image/png".to_string(), "image/jpeg".to_string()],
        supported_outputs: vec!["image/png".to_string()],
    }
}
