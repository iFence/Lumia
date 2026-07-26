use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use async_channel::{Receiver, Sender};
use gpui::{Context, Window};
use serde::{Deserialize, Serialize};

use crate::app::LumiaApp;

const MAX_REQUEST_BYTES: usize = 1024 * 1024;
const ACK: u8 = 0x4c;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "path", rename_all = "snake_case")]
enum InstanceRequest {
    Activate,
    OpenFile(PathBuf),
}

pub(crate) struct PrimaryInstance {
    receiver: Receiver<InstanceRequest>,
    _guard: platform::Guard,
}

pub(crate) fn acquire(initial_path: Option<&Path>) -> anyhow::Result<Option<PrimaryInstance>> {
    let request = initial_path
        .map(Path::to_path_buf)
        .map(InstanceRequest::OpenFile)
        .unwrap_or(InstanceRequest::Activate);
    let (sender, receiver) = async_channel::unbounded();

    platform::acquire(&request, sender).map(|guard| {
        guard.map(|guard| PrimaryInstance {
            receiver,
            _guard: guard,
        })
    })
}

impl LumiaApp {
    pub(crate) fn listen_for_instance_requests(
        &mut self,
        primary_instance: PrimaryInstance,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        cx.spawn_in(window, async move |this, cx| {
            while let Ok(request) = primary_instance.receiver.recv().await {
                let result = this.update_in(cx, |this, window, cx| {
                    window.activate_window();
                    if let InstanceRequest::OpenFile(path) = request {
                        this.load_image(path, Some(window), cx);
                    }
                    cx.notify();
                });
                if result.is_err() {
                    break;
                }
            }
        })
        .detach();
    }
}

fn write_request(stream: &mut impl Write, request: &InstanceRequest) -> io::Result<()> {
    let payload = serde_json::to_vec(request).map_err(io::Error::other)?;
    if payload.len() > MAX_REQUEST_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "single-instance request is too large",
        ));
    }
    let length = u32::try_from(payload.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "request length overflow"))?;
    stream.write_all(&length.to_le_bytes())?;
    stream.write_all(&payload)?;
    stream.flush()
}

fn read_request(stream: &mut impl Read) -> io::Result<InstanceRequest> {
    let mut length = [0_u8; 4];
    stream.read_exact(&mut length)?;
    let length = u32::from_le_bytes(length) as usize;
    if length > MAX_REQUEST_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "single-instance request is too large",
        ));
    }
    let mut payload = vec![0; length];
    stream.read_exact(&mut payload)?;
    serde_json::from_slice(&payload).map_err(io::Error::other)
}

fn serve_connection(
    stream: &mut (impl Read + Write),
    sender: &Sender<InstanceRequest>,
) -> io::Result<bool> {
    let request = read_request(stream)?;
    if sender.send_blocking(request).is_err() {
        return Ok(false);
    }
    stream.write_all(&[ACK])?;
    stream.flush()?;
    Ok(true)
}

#[cfg(not(target_os = "windows"))]
#[path = "single_instance/non_windows.rs"]
mod platform;
#[cfg(target_os = "windows")]
#[path = "single_instance/windows.rs"]
mod platform;

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[cfg(target_os = "windows")]
    const FORWARD_TEST_PATH_ENV: &str = "LUMIA_SINGLE_INSTANCE_TEST_PATH";

    #[test]
    fn request_frame_round_trips_file_paths() {
        let request = InstanceRequest::OpenFile(PathBuf::from("fixtures/photo.png"));
        let mut bytes = Vec::new();
        write_request(&mut bytes, &request).unwrap();

        assert_eq!(read_request(&mut Cursor::new(bytes)).unwrap(), request);
    }

    #[test]
    fn oversized_request_is_rejected_before_allocating_payload() {
        let mut bytes = ((MAX_REQUEST_BYTES + 1) as u32).to_le_bytes().to_vec();
        bytes.extend_from_slice(b"ignored");

        let error = read_request(&mut Cursor::new(bytes)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn second_process_forwards_file_to_primary_instance() {
        use std::process::Command;

        let primary = acquire(None).unwrap().expect("test owns primary instance");
        let expected_path = PathBuf::from(r"C:\pictures\forwarded image.png");
        let status = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "single_instance::tests::secondary_process_forwarding_helper",
            ])
            .env(FORWARD_TEST_PATH_ENV, &expected_path)
            .status()
            .unwrap();

        assert!(status.success());
        assert_eq!(
            primary.receiver.recv_blocking().unwrap(),
            InstanceRequest::OpenFile(expected_path)
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "launched by second_process_forwards_file_to_primary_instance"]
    fn secondary_process_forwarding_helper() {
        let path = std::env::var_os(FORWARD_TEST_PATH_ENV)
            .map(PathBuf::from)
            .expect("forwarding helper requires a path");

        assert!(acquire(Some(&path)).unwrap().is_none());
    }
}
