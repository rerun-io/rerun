//! TODO(emilk): use tokio instead

use std::sync::mpsc::{Receiver, Sender};

use re_log_types::LogMsg;

/// ```
/// let log_msg_rx = serve("127.0.0.1:80")?;
/// ```
pub fn serve(addr: impl std::net::ToSocketAddrs) -> anyhow::Result<Receiver<LogMsg>> {
    let listener = std::net::TcpListener::bind(addr)?;

    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::Builder::new()
        .name("sdk-server".into())
        .spawn(move || {
            // accept connections and process them serially
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        let tx = tx.clone();
                        handle_client(stream, tx);
                    }
                    Err(err) => {
                        tracing::warn!("Failed to accept incoming SDK client: {err:?}");
                    }
                }
            }
        })
        .expect("Failed to spawn thread");

    Ok(rx)
}

fn handle_client(stream: std::net::TcpStream, tx: Sender<LogMsg>) {
    std::thread::Builder::new()
        .name("sdk-server-client-handler".into())
        .spawn(move || {
            tracing::info!("New SDK client connected: {:?}", stream.peer_addr());

            if let Err(err) = run_client(stream, &tx) {
                tracing::warn!("Closing connection to client: {err:?}");
            }
        })
        .expect("Failed to spawn thread");
}

fn run_client(mut stream: std::net::TcpStream, tx: &Sender<LogMsg>) -> anyhow::Result<()> {
    use std::io::Read as _;

    let mut client_version = [0_u8; 2];
    stream.read_exact(&mut client_version)?;
    let client_version = u16::from_le_bytes(client_version);

    match client_version.cmp(&crate::PROTOCOL_VERSION) {
        std::cmp::Ordering::Less => {
            anyhow::bail!(
                "sdk client is using an older protocol version ({}) than the sdk server ({}).",
                client_version,
                crate::PROTOCOL_VERSION
            );
        }
        std::cmp::Ordering::Equal => {}
        std::cmp::Ordering::Greater => {
            anyhow::bail!(
                "sdk client is using a newer protocol version ({}) than the sdk server ({}).",
                client_version,
                crate::PROTOCOL_VERSION
            );
        }
    }

    let mut packet = Vec::new();

    loop {
        let mut packet_size = [0_u8; 4];
        stream.read_exact(&mut packet_size)?;
        let packet_size = u32::from_le_bytes(packet_size);

        packet.resize(packet_size as usize, 0_u8);
        stream.read_exact(&mut packet)?;

        tracing::trace!("Received log message of size {packet_size}.");

        let msg = crate::decode_log_msg(&packet)?;

        tx.send(msg)?;
    }
}
