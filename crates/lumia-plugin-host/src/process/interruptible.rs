use std::io::Write;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use lumia_plugin_api::{JsonRpcRequest, JsonRpcResponse, RpcId, JSON_RPC_VERSION};
use serde::{de::DeserializeOwned, Serialize};

use crate::{PluginHostError, Result};

use super::PluginProcess;

const INTERRUPT_POLL_INTERVAL: Duration = Duration::from_millis(25);

impl PluginProcess {
    pub fn request_interruptible<P, R, F>(
        &mut self,
        method: &str,
        params: P,
        timeout: Duration,
        is_cancelled: F,
    ) -> Result<R>
    where
        P: Serialize,
        R: DeserializeOwned,
        F: Fn() -> bool,
    {
        if is_cancelled() {
            self.terminate();
            return Err(PluginHostError::Cancelled);
        }

        let id = RpcId::Number(self.next_id);
        self.next_id += 1;
        let params = serde_json::to_value(params).map_err(PluginHostError::Serialize)?;
        let request = JsonRpcRequest::new(id.clone(), method, params);
        let mut line = serde_json::to_string(&request).map_err(PluginHostError::Serialize)?;
        line.push('\n');
        self.stdin.write_all(line.as_bytes())?;
        self.stdin.flush()?;

        let deadline = Instant::now() + timeout;
        let response_line = loop {
            if is_cancelled() {
                self.terminate();
                return Err(PluginHostError::Cancelled);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                self.terminate();
                return Err(PluginHostError::ResponseTimeout {
                    seconds: timeout.as_secs(),
                });
            }
            match self
                .responses
                .recv_timeout(remaining.min(INTERRUPT_POLL_INTERVAL))
            {
                Ok(Ok(line)) => break line,
                Ok(Err(error)) if error.kind() == std::io::ErrorKind::InvalidData => {
                    self.terminate();
                    return Err(PluginHostError::InvalidResponseBody);
                }
                Ok(Err(error)) => {
                    self.terminate();
                    return Err(error.into());
                }
                Err(RecvTimeoutError::Timeout) if Instant::now() < deadline => continue,
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
}
