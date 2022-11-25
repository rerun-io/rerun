//! TODO(emilk): use tokio instead

use re_log_types::LogMsg;
use re_smart_channel::{Receiver, Sender};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ServerOptions {
    /// If the latency in the [`LogMsg`] channel is greater than this,
    /// then start dropping messages in order to keep up.
    pub max_latency_sec: f32,
}

/// Listen to multiple SDK:s connecting to us over TCP.
///
/// ``` no_run
/// # use re_sdk_comms::serve;
/// let log_msg_rx = serve("127.0.0.1:80")?;
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn serve(
    addr: impl std::net::ToSocketAddrs,
    options: ServerOptions,
) -> anyhow::Result<Receiver<LogMsg>> {
    let listener = std::net::TcpListener::bind(addr)?;

    let (tx, rx) = re_smart_channel::smart_channel(re_smart_channel::Source::Network);

    std::thread::Builder::new()
        .name("sdk-server".into())
        .spawn(move || {
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        let tx = tx.clone();
                        spawn_client(stream, tx, options);
                    }
                    Err(err) => {
                        re_log::warn!("Failed to accept incoming SDK client: {err:?}");
                    }
                }
            }
        })
        .expect("Failed to spawn thread");

    Ok(rx)
}

fn spawn_client(stream: std::net::TcpStream, tx: Sender<LogMsg>, options: ServerOptions) {
    std::thread::Builder::new()
        .name(format!(
            "sdk-server-client-handler-{:?}",
            stream.peer_addr()
        ))
        .spawn(move || {
            re_log::info!("New SDK client connected: {:?}", stream.peer_addr());

            if let Err(err) = run_client(stream, &tx, options) {
                re_log::warn!("Closing connection to client: {err:?}");
            }
        })
        .expect("Failed to spawn thread");
}

fn run_client(
    mut stream: std::net::TcpStream,
    tx: &Sender<LogMsg>,
    options: ServerOptions,
) -> anyhow::Result<()> {
    #![allow(clippy::read_zero_byte_vec)] // false positive: https://github.com/rust-lang/rust-clippy/issues/9274

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

        re_log::trace!("Received log message of size {packet_size}.");

        if tx.latency_sec() > options.max_latency_sec {
            // TODO: we don't get new and improved latency numbers if we drop _all_ packets.
            re_log::warn_once!(
                "Input latency is over the max ({} s) - dropping packets.",
                options.max_latency_sec
            );
        } else {
            let msg = crate::decode_log_msg(&packet)?;
            tx.send(msg)?;
        }
    }
}
