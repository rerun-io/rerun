//! TODO(emilk): use tokio instead

use std::time::Instant;

use rand::{Rng, SeedableRng};
use re_log_types::LogMsg;
use re_smart_channel::{Receiver, Sender};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ServerOptions {
    /// If the latency in the [`LogMsg`] channel is greater than this,
    /// then start dropping messages in order to keep up.
    pub max_latency_sec: f32,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            max_latency_sec: f32::INFINITY,
        }
    }
}

/// Listen to multiple SDK:s connecting to us over TCP.
///
/// ``` no_run
/// # use re_sdk_comms::{serve, ServerOptions};
/// let log_msg_rx = serve("127.0.0.1:80", ServerOptions::default())?;
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

    let mut congestion_manager = CongestionManager::new(options.max_latency_sec);

    let mut packet = Vec::new();

    loop {
        let mut packet_size = [0_u8; 4];
        stream.read_exact(&mut packet_size)?;
        let packet_size = u32::from_le_bytes(packet_size);

        packet.resize(packet_size as usize, 0_u8);
        stream.read_exact(&mut packet)?;

        re_log::trace!("Received log message of size {packet_size}.");

        if congestion_manager.accepts(tx.latency_sec()) {
            let msg = crate::decode_log_msg(&packet)?;
            tx.send(msg)?;
        } else {
            re_log::warn_once!(
                "Input latency is over the max ({} s) - dropping packets.",
                options.max_latency_sec
            );
        }
    }
}

/// Decides how many messages to drop so that we achieve a desired maximum latency.
struct CongestionManager {
    max_latency_sec: f32,
    accept_rate: f32,
    last_time: Instant,
    rng: rand::rngs::SmallRng,
}

impl CongestionManager {
    pub fn new(max_latency_sec: f32) -> Self {
        Self {
            max_latency_sec,
            accept_rate: 1.0,
            last_time: Instant::now(),
            rng: rand::rngs::SmallRng::from_entropy(),
        }
    }

    pub fn accepts(&mut self, current_latency: f32) -> bool {
        let now = Instant::now();
        let dt = (now - self.last_time).as_secs_f32();
        self.last_time = now;

        let is_good = current_latency < self.max_latency_sec;

        if is_good && self.accept_rate == 1.0 {
            return true; // early out
        }

        /// If we let it go too low, we won't accept any messages,
        /// and then we won't ever recover.
        const MIN_ACCPET_RATE: f32 = 0.01;

        // This is quite ad-hoc, but better than nothing.
        // Perhaps it's worth investigating a more rigorous additive increase/multiplicative decrease congestion protocol.
        if is_good {
            // Slowly improve our accept-rate, slower the closer we are:
            let goodness = (self.max_latency_sec - current_latency) / self.max_latency_sec;
            self.accept_rate += goodness * dt / 25.0;
        } else {
            // Quickly decrease our accept-rate, quicker the worse we are:
            let badness = (current_latency - self.max_latency_sec) / self.max_latency_sec;
            let badness = badness.clamp(0.5, 2.0);
            self.accept_rate -= badness * dt / 15.0;
        }

        self.accept_rate = self.accept_rate.clamp(MIN_ACCPET_RATE, 1.0);

        self.rng.gen::<f32>() < self.accept_rate
    }
}
