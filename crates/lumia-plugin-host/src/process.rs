use std::collections::HashSet;
use std::io::{BufRead, BufReader, Write};
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use lumia_plugin_api::{
    EmptyResult, InitializeParams, InitializeResult, JsonRpcRequest, JsonRpcResponse,
    PluginCapability, PluginManifest, PluginPermission, RpcId, JSON_RPC_VERSION, PROTOCOL_VERSION,
};
use serde::{de::DeserializeOwned, Serialize};

use crate::{PluginHostError, Result};
mod interruptible;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const MAX_RPC_RESPONSE_BYTES: usize = 1024 * 1024;
const CONTROL_RESPONSE_TIMEOUT: Duration = Duration::from_secs(5);
const TASK_RESPONSE_TIMEOUT: Duration = Duration::from_secs(120);

pub struct PluginProcess {
    child: Child,
    stdin: ChildStdin,
    responses: Receiver<std::io::Result<String>>,
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
            .spawn()
            .map_err(PluginHostError::Spawn)?;
        let Some(stdin) = child.stdin.take() else {
            terminate_child(&mut child);
            return Err(PluginHostError::MissingStdin);
        };
        let Some(stdout) = child.stdout.take() else {
            terminate_child(&mut child);
            return Err(PluginHostError::MissingStdout);
        };
        let responses = match spawn_response_reader(stdout) {
            Ok(responses) => responses,
            Err(error) => {
                terminate_child(&mut child);
                return Err(error);
            }
        };

        Ok(Self {
            child,
            stdin,
            responses,
            next_id: 1,
        })
    }

    pub fn initialize(&mut self) -> Result<InitializeResult> {
        self.request_with_timeout(
            "plugin.initialize",
            InitializeParams::default(),
            CONTROL_RESPONSE_TIMEOUT,
        )
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
        self.request_with_timeout(method, params, TASK_RESPONSE_TIMEOUT)
    }

    pub fn request_with_timeout<P, R>(
        &mut self,
        method: &str,
        params: P,
        timeout: Duration,
    ) -> Result<R>
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

        let response_line = match self.responses.recv_timeout(timeout) {
            Ok(Ok(line)) => line,
            Ok(Err(error)) if error.kind() == std::io::ErrorKind::InvalidData => {
                self.terminate();
                return Err(PluginHostError::InvalidResponseBody);
            }
            Ok(Err(error)) => {
                self.terminate();
                return Err(error.into());
            }
            Err(RecvTimeoutError::Timeout) => {
                self.terminate();
                return Err(PluginHostError::ResponseTimeout {
                    seconds: timeout.as_secs(),
                });
            }
            Err(RecvTimeoutError::Disconnected) => {
                self.terminate();
                return Err(PluginHostError::Closed);
            }
        };
        let response: JsonRpcResponse = match serde_json::from_str(&response_line) {
            Ok(response) => response,
            Err(error) => {
                self.terminate();
                return Err(PluginHostError::Deserialize(error));
            }
        };
        if response.jsonrpc != JSON_RPC_VERSION {
            self.terminate();
            return Err(PluginHostError::JsonRpcVersion {
                expected: JSON_RPC_VERSION,
                actual: response.jsonrpc,
            });
        }
        if response.id != id {
            self.terminate();
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

        match serde_json::from_value(response.result.unwrap_or_default()) {
            Ok(result) => Ok(result),
            Err(error) => {
                self.terminate();
                Err(PluginHostError::Deserialize(error))
            }
        }
    }

    pub fn child_id(&self) -> u32 {
        self.child.id()
    }

    pub fn shutdown(mut self) -> Result<()> {
        let _: EmptyResult = self.request_with_timeout(
            "plugin.shutdown",
            EmptyResult::default(),
            CONTROL_RESPONSE_TIMEOUT,
        )?;
        Ok(())
    }

    pub fn terminate(&mut self) {
        terminate_child(&mut self.child);
    }
}

impl Drop for PluginProcess {
    fn drop(&mut self) {
        terminate_child(&mut self.child);
    }
}

#[cfg(windows)]
fn configure_plugin_command(command: &mut Command) {
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn configure_plugin_command(_: &mut Command) {}

fn terminate_child(child: &mut Child) {
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn spawn_response_reader(stdout: ChildStdout) -> Result<Receiver<std::io::Result<String>>> {
    let (sender, receiver) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("lumia-plugin-response".into())
        .spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                match read_bounded_line(&mut reader, MAX_RPC_RESPONSE_BYTES) {
                    Ok(Some(line)) => {
                        if sender.send(Ok(line)).is_err() {
                            break;
                        }
                    }
                    Ok(None) => break,
                    Err(error) => {
                        let _ = sender.send(Err(error));
                        break;
                    }
                }
            }
        })
        .map_err(PluginHostError::Spawn)?;
    Ok(receiver)
}

fn read_bounded_line(
    reader: &mut impl BufRead,
    maximum_bytes: usize,
) -> std::io::Result<Option<String>> {
    let mut bytes = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            if bytes.is_empty() {
                return Ok(None);
            }
            break;
        }
        let take = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |position| position + 1);
        if bytes.len().saturating_add(take) > maximum_bytes {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "plugin response is too large",
            ));
        }
        bytes.extend_from_slice(&available[..take]);
        reader.consume(take);
        if bytes.last() == Some(&b'\n') {
            break;
        }
    }
    String::from_utf8(bytes)
        .map(Some)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

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
    validate_supported_extensions(manifest)?;
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

pub fn validate_supported_extensions(manifest: &PluginManifest) -> Result<()> {
    const MAX_EXTENSIONS: usize = 128;
    const MAX_EXTENSION_LENGTH: usize = 16;

    if manifest.supported_extensions.len() > MAX_EXTENSIONS {
        return Err(PluginHostError::InvalidManifest(format!(
            "supported_extensions exceeds {MAX_EXTENSIONS} entries"
        )));
    }
    let mut seen = HashSet::new();
    for extension in &manifest.supported_extensions {
        let valid = !extension.is_empty()
            && extension.len() <= MAX_EXTENSION_LENGTH
            && extension
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit());
        if !valid {
            return Err(PluginHostError::InvalidManifest(format!(
                "invalid supported extension: {extension}"
            )));
        }
        if !seen.insert(extension) {
            return Err(PluginHostError::InvalidManifest(format!(
                "duplicate supported extension: {extension}"
            )));
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
            supported_extensions: vec!["psd".to_string(), "psb".to_string()],
            supported_outputs: vec!["image/png".to_string()],
            contributions: Default::default(),
            assets: Vec::new(),
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

    #[test]
    fn response_reader_enforces_line_limit() {
        let mut valid = std::io::Cursor::new(b"{\"ok\":true}\n".as_slice());
        assert_eq!(
            read_bounded_line(&mut valid, 32).unwrap().as_deref(),
            Some("{\"ok\":true}\n")
        );

        let mut oversized = std::io::Cursor::new(b"123456\n".as_slice());
        assert!(read_bounded_line(&mut oversized, 4).is_err());
    }

    #[test]
    fn decoder_extensions_are_lowercase_unique_and_bounded() {
        let valid = manifest();
        validate_decode_preview_manifest(&valid).unwrap();

        let mut uppercase = valid.clone();
        uppercase.supported_extensions = vec!["DNG".into()];
        assert!(validate_decode_preview_manifest(&uppercase).is_err());

        let mut dotted = valid.clone();
        dotted.supported_extensions = vec![".dng".into()];
        assert!(validate_decode_preview_manifest(&dotted).is_err());

        let mut duplicate = valid;
        duplicate.supported_extensions = vec!["dng".into(), "dng".into()];
        assert!(validate_decode_preview_manifest(&duplicate).is_err());
    }
}
