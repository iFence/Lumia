use std::ffi::OsStr;
use std::fs::{File, OpenOptions};
use std::io::{self, Read as _};
use std::os::windows::ffi::OsStrExt as _;
use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _};
use std::ptr;
use std::thread;
use std::time::Duration;

use anyhow::Context as _;
use async_channel::Sender;
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, ERROR_PIPE_CONNECTED, INVALID_HANDLE_VALUE,
};
use windows_sys::Win32::Storage::FileSystem::PIPE_ACCESS_DUPLEX;
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE,
    PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows_sys::Win32::System::Threading::{CreateMutexW, GetCurrentProcessId};

use super::{serve_connection, write_request, InstanceRequest, ACK};

const NAME: &str = "Lumia.SingleInstance.v1";
const PIPE_BUFFER_BYTES: u32 = 64 * 1024;
const FORWARD_ATTEMPTS: usize = 80;
const FORWARD_RETRY_DELAY: Duration = Duration::from_millis(25);

pub(super) struct Guard {
    mutex: windows_sys::Win32::Foundation::HANDLE,
}

impl Drop for Guard {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.mutex);
        }
    }
}

pub(super) fn acquire(
    request: &InstanceRequest,
    sender: Sender<InstanceRequest>,
) -> anyhow::Result<Option<Guard>> {
    acquire_named(NAME, request, sender)
}

pub(super) fn acquire_named(
    name: &str,
    request: &InstanceRequest,
    sender: Sender<InstanceRequest>,
) -> anyhow::Result<Option<Guard>> {
    let session_id = current_session_id()?;
    let mutex_name = wide(&format!(r"Local\{name}.{session_id}"));
    let pipe_name = format!(r"\\.\pipe\{name}.{session_id}");

    let mutex = unsafe { CreateMutexW(ptr::null(), 0, mutex_name.as_ptr()) };
    if mutex.is_null() {
        return Err(io::Error::last_os_error()).context("failed to create instance mutex");
    }

    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe {
            CloseHandle(mutex);
        }
        forward_with_retry(&pipe_name, request)?;
        return Ok(None);
    }

    let guard = Guard { mutex };
    thread::Builder::new()
        .name("lumia-instance-listener".into())
        .spawn(move || listen(pipe_name, sender))
        .context("failed to start instance listener")?;
    Ok(Some(guard))
}

fn current_session_id() -> io::Result<u32> {
    let mut session_id = 0;
    let success = unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut session_id) };
    if success == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(session_id)
    }
}

fn wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}

fn listen(pipe_name: String, sender: Sender<InstanceRequest>) {
    while !sender.is_closed() {
        match serve_one(&pipe_name, &sender) {
            Ok(true) => {}
            Ok(false) => break,
            Err(_) => thread::sleep(Duration::from_millis(100)),
        }
    }
}

fn serve_one(pipe_name: &str, sender: &Sender<InstanceRequest>) -> io::Result<bool> {
    let pipe_name = wide(pipe_name);
    let pipe = unsafe {
        CreateNamedPipeW(
            pipe_name.as_ptr(),
            PIPE_ACCESS_DUPLEX,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            PIPE_BUFFER_BYTES,
            PIPE_BUFFER_BYTES,
            0,
            ptr::null(),
        )
    };
    if pipe == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }

    let connected = unsafe { ConnectNamedPipe(pipe, ptr::null_mut()) };
    if connected == 0 && unsafe { GetLastError() } != ERROR_PIPE_CONNECTED {
        unsafe {
            CloseHandle(pipe);
        }
        return Err(io::Error::last_os_error());
    }

    let mut stream = unsafe { File::from_raw_handle(pipe) };
    let result = serve_connection(&mut stream, sender);
    unsafe {
        DisconnectNamedPipe(stream.as_raw_handle());
    }
    result
}

fn forward_with_retry(pipe_name: &str, request: &InstanceRequest) -> anyhow::Result<()> {
    let mut last_error = None;
    for _ in 0..FORWARD_ATTEMPTS {
        match OpenOptions::new().read(true).write(true).open(pipe_name) {
            Ok(mut stream) => {
                write_request(&mut stream, request)
                    .context("failed to send request to the running Lumia instance")?;
                let mut ack = [0_u8; 1];
                stream
                    .read_exact(&mut ack)
                    .context("running Lumia instance did not acknowledge the request")?;
                anyhow::ensure!(
                    ack[0] == ACK,
                    "running Lumia instance returned an invalid reply"
                );
                return Ok(());
            }
            Err(error) => {
                last_error = Some(error);
                thread::sleep(FORWARD_RETRY_DELAY);
            }
        }
    }
    Err(last_error.unwrap_or_else(|| io::Error::other("instance pipe unavailable")))
        .context("failed to connect to the running Lumia instance")
}
