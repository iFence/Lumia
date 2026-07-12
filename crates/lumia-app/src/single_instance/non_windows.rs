use std::io::{self, Read as _};
use std::net::{TcpListener, TcpStream};
use std::thread;
use std::time::Duration;

use anyhow::Context as _;
use async_channel::Sender;

use super::{serve_connection, write_request, InstanceRequest, ACK};

const ADDRESS: (&str, u16) = ("127.0.0.1", 47831);

pub(super) struct Guard;

pub(super) fn acquire(
    request: &InstanceRequest,
    sender: Sender<InstanceRequest>,
) -> anyhow::Result<Option<Guard>> {
    match TcpListener::bind(ADDRESS) {
        Ok(listener) => {
            thread::Builder::new()
                .name("lumia-instance-listener".into())
                .spawn(move || listen(listener, sender))
                .context("failed to start instance listener")?;
            Ok(Some(Guard))
        }
        Err(error) if error.kind() == io::ErrorKind::AddrInUse => {
            forward_with_retry(request)?;
            Ok(None)
        }
        Err(error) => Err(error).context("failed to acquire single-instance endpoint"),
    }
}

fn listen(listener: TcpListener, sender: Sender<InstanceRequest>) {
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else {
            continue;
        };
        match serve_connection(&mut stream, &sender) {
            Ok(true) => {}
            Ok(false) => break,
            Err(_) => {}
        }
    }
}

fn forward_with_retry(request: &InstanceRequest) -> anyhow::Result<()> {
    let mut last_error = None;
    for _ in 0..80 {
        match TcpStream::connect(ADDRESS) {
            Ok(mut stream) => {
                write_request(&mut stream, request)?;
                let mut ack = [0_u8; 1];
                stream.read_exact(&mut ack)?;
                anyhow::ensure!(
                    ack[0] == ACK,
                    "running Lumia instance returned an invalid reply"
                );
                return Ok(());
            }
            Err(error) => {
                last_error = Some(error);
                thread::sleep(Duration::from_millis(25));
            }
        }
    }
    Err(last_error.unwrap_or_else(|| io::Error::other("instance endpoint unavailable")))
        .context("failed to connect to the running Lumia instance")
}
