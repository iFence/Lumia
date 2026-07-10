use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use lumia_plugin_api::{
    InitializeParams, InitializeResult, JsonRpcRequest, JsonRpcResponse, PluginManifest, RpcId,
};
use serde::{de::DeserializeOwned, Serialize};

use crate::{PluginHostError, Result};

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

        let mut child = Command::new(&manifest.entry)
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
        if let Some(error) = response.error {
            return Err(PluginHostError::Rpc {
                code: error.code,
                message: error.message,
            });
        }

        serde_json::from_value(response.result.unwrap_or_default())
            .map_err(PluginHostError::Deserialize)
    }

    pub fn child_id(&self) -> u32 {
        self.child.id()
    }
}

#[cfg(test)]
mod tests {
    use lumia_plugin_api::{InitializeParams, PROTOCOL_VERSION};

    #[test]
    fn initialize_params_serialize_protocol_version() {
        let value = serde_json::to_value(InitializeParams::default()).unwrap();
        assert_eq!(value["protocol_version"], PROTOCOL_VERSION);
    }
}
