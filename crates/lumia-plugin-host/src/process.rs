use std::io::{BufRead, BufReader, Write};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use lumia_plugin_api::{
    InitializeParams, InitializeResult, JsonRpcRequest, JsonRpcResponse, PluginCapability,
    PluginManifest, PluginPermission, RpcId, JSON_RPC_VERSION, PROTOCOL_VERSION,
};
use serde::{de::DeserializeOwned, Serialize};

use crate::{PluginHostError, Result};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub struct PluginProcess {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl PluginProcess {
    pub fn spawn(manifest: &PluginManifest) -> Result<Self> {
        if manifest.entry.as_os_str().is_empty() {
            return Err(PluginHostError::EmptyEntry);
        }

        let mut command = Command::new(&manifest.entry);
        configure_plugin_command(&mut command);

        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let stdin = child.stdin.take().ok_or(PluginHostError::MissingStdin)?;
        let stdout = child.stdout.take().ok_or(PluginHostError::MissingStdout)?;

        Ok(Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
            next_id: 1,
        })
    }

    pub fn initialize(&mut self) -> Result<InitializeResult> {
        self.request("plugin.initialize", InitializeParams::default())
    }

    pub fn initialize_for(&mut self, expected: &PluginManifest) -> Result<InitializeResult> {
        let result = self.initialize()?;
        validate_initialize(expected, &result)?;
        Ok(result)
    }

    pub fn request<P, R>(&mut self, method: &str, params: P) -> Result<R>
    where
        P: Serialize,
        R: DeserializeOwned,
    {
        let id = RpcId::Number(self.next_id);
        self.next_id += 1;

        let params = serde_json::to_value(params).map_err(PluginHostError::Serialize)?;
        let request = JsonRpcRequest::new(id.clone(), method, params);
        let mut line = serde_json::to_string(&request).map_err(PluginHostError::Serialize)?;
        line.push('\n');
        self.stdin.write_all(line.as_bytes())?;
        self.stdin.flush()?;

        let mut response_line = String::new();
        if self.stdout.read_line(&mut response_line)? == 0 {
            return Err(PluginHostError::Closed);
        }
        let response: JsonRpcResponse =
            serde_json::from_str(&response_line).map_err(PluginHostError::Deserialize)?;
        if response.jsonrpc != JSON_RPC_VERSION {
            return Err(PluginHostError::JsonRpcVersion {
                expected: JSON_RPC_VERSION,
                actual: response.jsonrpc,
            });
        }
        if response.id != id {
            return Err(PluginHostError::ResponseId);
        }
        if let Some(error) = response.error {
            let kind = error.plugin_kind();
            return Err(PluginHostError::Rpc {
                code: error.code,
                message: error.message,
                kind,
            });
        }

        serde_json::from_value(response.result.unwrap_or_default())
            .map_err(PluginHostError::Deserialize)
    }

    pub fn child_id(&self) -> u32 {
        self.child.id()
    }
}

#[cfg(windows)]
fn configure_plugin_command(command: &mut Command) {
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn configure_plugin_command(_: &mut Command) {}

pub fn validate_initialize(expected: &PluginManifest, result: &InitializeResult) -> Result<()> {
    if result.protocol_version != PROTOCOL_VERSION {
        return Err(PluginHostError::ProtocolMismatch {
            expected: PROTOCOL_VERSION,
            actual: result.protocol_version,
        });
    }
    if result.manifest.id != expected.id {
        return Err(PluginHostError::PluginIdMismatch {
            expected: expected.id.clone(),
            actual: result.manifest.id.clone(),
        });
    }
    for capability in &expected.capabilities {
        if !result.manifest.capabilities.contains(capability) {
            return Err(PluginHostError::MissingCapability(*capability));
        }
    }
    Ok(())
}

pub fn validate_decode_preview_manifest(manifest: &PluginManifest) -> Result<()> {
    for capability in [PluginCapability::Probe, PluginCapability::DecodePreview] {
        if !manifest.capabilities.contains(&capability) {
            return Err(PluginHostError::MissingCapability(capability));
        }
    }
    for permission in [
        PluginPermission::ReadInputPath,
        PluginPermission::WriteTemporaryOutput,
    ] {
        if !manifest.permissions.contains(&permission) {
            return Err(PluginHostError::MissingPermission(permission));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lumia_plugin_api::{
        InitializeParams, InitializeResult, PluginCapability, PluginManifest, PluginPermission,
        PROTOCOL_VERSION,
    };

    #[test]
    fn initialize_params_serialize_protocol_version() {
        let value = serde_json::to_value(InitializeParams::default()).unwrap();
        assert_eq!(value["protocol_version"], PROTOCOL_VERSION);
    }

    fn manifest() -> PluginManifest {
        PluginManifest {
            id: "lumia.photoshop".to_string(),
            name: "Photoshop".to_string(),
            version: "0.1.0".to_string(),
            entry: "lumia-plugin-photoshop".into(),
            capabilities: vec![PluginCapability::Probe, PluginCapability::DecodePreview],
            permissions: vec![
                PluginPermission::ReadInputPath,
                PluginPermission::WriteTemporaryOutput,
            ],
            supported_inputs: vec!["image/vnd.adobe.photoshop".to_string()],
            supported_outputs: vec!["image/png".to_string()],
        }
    }

    #[test]
    fn initialize_validation_rejects_protocol_mismatch() {
        let expected = manifest();
        let result = InitializeResult {
            protocol_version: PROTOCOL_VERSION + 1,
            manifest: expected.clone(),
        };

        assert!(validate_initialize(&expected, &result).is_err());
    }

    #[test]
    fn preview_validation_requires_declared_permissions() {
        let mut manifest = manifest();
        manifest.permissions.clear();

        assert!(validate_decode_preview_manifest(&manifest).is_err());
    }
}
