use std::{
    io::{ErrorKind, Read as _},
    net::{TcpListener, TcpStream},
    time::Instant,
};

use rand::{Rng as _, SeedableRng};

use re_log_types::{LogMsg, TimePoint, TimeType, TimelineName};
use re_smart_channel::{Receiver, Sender};

use crate::{ConnectionError, VersionError};

#[derive(thiserror::Error, Debug)]
pub enum ServerError {
    #[error("Failed to bind TCP address {bind_addr:?}. Another Rerun instance is probably running. {err}")]
    TcpBindError {
        bind_addr: String,
        err: std::io::Error,
    },

    #[error(transparent)]
    FailedToSpawnThread(#[from] std::io::Error),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ServerOptions {
    /// If the latency in the [`LogMsg`] channel is greater than this,
    /// then start dropping messages in order to keep up.
    pub max_latency_sec: f32,

    /// Turns `info`-level logs into `debug`-level logs.
    pub quiet: bool,
}

impl Default for ServerOptions {
    fn default() -> Self {
        Self {
            max_latency_sec: f32::INFINITY,
            quiet: false,
        }
    }
}

/// Listen to multiple SDK:s connecting to us over TCP.
///
/// ``` no_run
/// # use re_sdk_comms::{serve, ServerOptions};
/// fn main() {
///     let log_msg_rx = serve("0.0.0.0", re_sdk_comms::DEFAULT_SERVER_PORT, ServerOptions::default()).unwrap();
/// }
/// ```
///
/// Internally spawns a thread that listens for incoming TCP connections on the given `bind_ip` and `port`
/// and one thread per connected client.
// TODO(andreas): Reconsider if we should use `smol` tasks instead of threads both here and in re_ws_comms.
pub fn serve(
    bind_ip: &str,
    port: u16,
    options: ServerOptions,
) -> Result<Receiver<LogMsg>, ServerError> {
    let (tx, rx) = re_smart_channel::smart_channel(
        // NOTE: We don't know until we start actually accepting clients!
        re_smart_channel::SmartMessageSource::Unknown,
        re_smart_channel::SmartChannelSource::TcpServer { port },
    );

    let bind_addr = format!("{bind_ip}:{port}");
    let listener = TcpListener::bind(&bind_addr).map_err(|err| ServerError::TcpBindError {
        bind_addr: bind_addr.clone(),
        err,
    })?;

    std::thread::Builder::new()
        .name("rerun_sdk_comms: listener".to_owned())
        .spawn(move || {
            listen_for_new_clients(&listener, options, &tx);
        })?;

    if options.quiet {
        re_log::debug!(
            "Hosting a SDK server over TCP at {bind_addr}. Connect with the Rerun logging SDK."
        );
    } else {
        re_log::info!(
            "Hosting a SDK server over TCP at {bind_addr}. Connect with the Rerun logging SDK."
        );
    }

    Ok(rx)
}

fn listen_for_new_clients(listener: &TcpListener, options: ServerOptions, tx: &Sender<LogMsg>) {
    // TODO(emilk): some way of aborting this loop
    #[allow(clippy::infinite_loop)]
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                let addr = stream.peer_addr().ok();
                let tx = tx.clone_as(re_smart_channel::SmartMessageSource::TcpClient { addr });

                std::thread::Builder::new()
                    .name("rerun_sdk_comms: client".to_owned())
                    .spawn(move || {
                        spawn_client(stream, &tx, options, addr);
                    })
                    .ok();
            }
            Err(err) => {
                if cfg!(target_os = "windows") {
                    // Windows error codes resolved to names via http://errorcodelookup.com/
                    const WSANOTINITIALISED: i32 = 10093;
                    const WSAEINTR: i32 = 10004;

                    if let Some(raw_os_error) = err.raw_os_error() {
                        #[allow(clippy::match_same_arms)]
                        match raw_os_error {
                            WSANOTINITIALISED => {
                                // This happens either if WSAStartup wasn't called beforehand,
                                // or WSACleanup was called as part of shutdown already.
                                //
                                // If we end up in here it's almost certainly the later case
                                // which implies that the process is shutting down.
                                break;
                            }
                            WSAEINTR => {
                                // A blocking operation was interrupted.
                                // This can only happen if the listener is closing,
                                // meaning that this server is shutting down.
                                break;
                            }
                            _ => {}
                        }
                    }
                }

                re_log::warn!("Failed to accept incoming SDK client: {err}");
            }
        }
    }
}

fn spawn_client(
    stream: TcpStream,
    tx: &Sender<LogMsg>,
    options: ServerOptions,
    peer_addr: Option<std::net::SocketAddr>,
) {
    let addr_string = peer_addr.map_or_else(|| "(unknown ip)".to_owned(), |addr| addr.to_string());

    if let Err(err) = run_client(stream, &addr_string, tx, options) {
        if let ConnectionError::SendError(err) = &err {
            if err.kind() == ErrorKind::UnexpectedEof {
                // Client gracefully severed the connection.
                tx.quit(None).ok(); // best-effort at this point
                return;
            }
        }

        if matches!(&err, ConnectionError::UnknownClient) {
            // An unknown client that probably stumbled onto the wrong port.
            // Don't log as an error (https://github.com/rerun-io/rerun/issues/5883).
            re_log::debug!(
                "Rejected incoming connection from unknown client at {addr_string}: {err}"
            );
        } else {
            re_log::warn_once!("Closing connection to client at {addr_string}: {err}");
        }

        let err: Box<dyn std::error::Error + Send + Sync + 'static> = err.into();
        tx.quit(Some(err)).ok(); // best-effort at this point
    }
}

fn run_client(
    mut stream: TcpStream,
    addr_string: &str,
    tx: &Sender<LogMsg>,
    options: ServerOptions,
) -> Result<(), ConnectionError> {
    #![allow(clippy::read_zero_byte_vec)] // false positive: https://github.com/rust-lang/rust-clippy/issues/9274

    let mut client_version = [0_u8; 2];
    stream.read_exact(&mut client_version)?;
    let client_version = u16::from_le_bytes(client_version);

    // The server goes into a backward compat mode
    // if the client sends version 0
    if client_version == crate::PROTOCOL_VERSION_0 {
        // Backwards compatibility mode: no protocol header, otherwise the same as version 1.
        re_log::warn!("Client is using an old protocol version from before 0.16.");
    } else {
        // The protocol header was added in version 1
        let mut protocol_header = [0_u8; crate::PROTOCOL_HEADER.len()];
        stream.read_exact(&mut protocol_header)?;

        if std::str::from_utf8(&protocol_header) != Ok(crate::PROTOCOL_HEADER) {
            return Err(ConnectionError::UnknownClient);
        }

        if options.quiet {
            re_log::debug!("New SDK client connected from: {addr_string}");
        } else {
            re_log::info!("New SDK client connected from: {addr_string}");
        }

        let server_version = crate::PROTOCOL_VERSION_1;
        match client_version.cmp(&server_version) {
            std::cmp::Ordering::Less => {
                return Err(ConnectionError::VersionError(VersionError::ClientIsOlder {
                    client_version,
                    server_version,
                }));
            }
            std::cmp::Ordering::Equal => {}
            std::cmp::Ordering::Greater => {
                return Err(ConnectionError::VersionError(VersionError::ClientIsNewer {
                    client_version,
                    server_version,
                }));
            }
        }
    };

    let mut congestion_manager = CongestionManager::new(options.max_latency_sec);

    let mut packet = Vec::new();

    loop {
        let mut packet_size = [0_u8; 4];
        stream.read_exact(&mut packet_size)?;
        let packet_size = u32::from_le_bytes(packet_size);

        packet.resize(packet_size as usize, 0_u8);
        stream.read_exact(&mut packet)?;

        re_log::trace!("Received packet of size {packet_size}.");

        congestion_manager.register_latency(tx.latency_sec());

        let version_policy = re_log_encoding::decoder::VersionPolicy::Warn;
        for msg in re_log_encoding::decoder::decode_bytes(version_policy, &packet)? {
            if congestion_manager.should_send(&msg) {
                tx.send(msg)?;
            } else {
                re_log::warn_once!(
                    "Input latency is over the max ({} s) - dropping packets.",
                    options.max_latency_sec
                );
            }
        }
    }
}

// ----------------------------------------------------------------------------

/// Decides how many messages to drop so that we achieve a desired maximum latency.
struct CongestionManager {
    throttling: Throttling,
    rng: rand::rngs::SmallRng,
    timeline_histories: ahash::HashMap<TimelineName, TimelineThrottling>,
}

#[derive(Default)]
struct TimelineThrottling {
    chance_of_sending: f32,
    send_time: std::collections::BTreeMap<i64, bool>,
}

impl CongestionManager {
    pub fn new(max_latency_sec: f32) -> Self {
        Self {
            throttling: Throttling::new(max_latency_sec),
            rng: rand::rngs::SmallRng::from_entropy(),
            timeline_histories: Default::default(),
        }
    }

    pub fn register_latency(&mut self, latency_sec: f32) {
        self.throttling.register_latency(latency_sec);
    }

    pub fn should_send(&mut self, msg: &LogMsg) -> bool {
        if self.throttling.accept_rate == 1.0 {
            return true; // early out for common-case
        }

        #[allow(clippy::match_same_arms)]
        match msg {
            // we don't want to drop any of these
            LogMsg::SetStoreInfo(_) | LogMsg::BlueprintActivationCommand { .. } => true,

            LogMsg::ArrowMsg(_, arrow_msg) => self.should_send_time_point(&arrow_msg.timepoint_max),
        }
    }

    fn should_send_time_point(&mut self, time_point: &TimePoint) -> bool {
        for (timeline, time) in time_point.iter() {
            if timeline.typ() == TimeType::Sequence {
                // We want to accept everything from the same sequence (e.g. frame nr) or nothing.
                // See https://github.com/rerun-io/rerun/issues/430 for why.
                return self.should_send_time(*timeline.name(), time.as_i64());
            }
        }

        // There is no sequence timeline - just do stochastic filtering:
        self.rng.gen::<f32>() < self.throttling.accept_rate
    }

    fn should_send_time(&mut self, timeline: TimelineName, time: i64) -> bool {
        let timeline_history = self.timeline_histories.entry(timeline).or_default();
        match timeline_history.send_time.entry(time) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                // New time (e.g. frame nr)! Should we send messages in it?
                // We use dithering via error diffusion to decide!

                let send_it = 0.5 < timeline_history.chance_of_sending;
                entry.insert(send_it);

                if send_it {
                    // make it less likely we will send the next time:
                    timeline_history.chance_of_sending -= 1.0;
                } else {
                    // make it more likely we will send it next time:
                    timeline_history.chance_of_sending += self.throttling.accept_rate;
                }

                // Prune history so it doesn't grow too long.
                // This only matters if messages arrive out-of-order.
                // If we prune too much, we run the risk of taking a new (different)
                // decision on a time we've previously seen,
                // thus sending parts of a sequence-time instead of all-or-nothing.
                while timeline_history.send_time.len() > 1024 {
                    let oldest_time = *timeline_history
                        .send_time
                        .keys()
                        .next()
                        .expect("safe because checked above");
                    timeline_history.send_time.remove(&oldest_time);
                }

                re_log::trace!("Send {timeline} {time}: {send_it}");

                send_it
            }
            std::collections::btree_map::Entry::Occupied(entry) => {
                *entry.get() // Reuse previous decision
            }
        }
    }
}

// ----------------------------------------------------------------------------

/// Figures out how large fraction of messages to send based on
/// the current latency vs our desired max latency.
struct Throttling {
    max_latency_sec: f32,
    accept_rate: f32,
    last_time: Instant,
    last_log_time: Instant,
}

impl Throttling {
    pub fn new(max_latency_sec: f32) -> Self {
        Self {
            max_latency_sec,
            accept_rate: 1.0,
            last_time: Instant::now(),
            last_log_time: Instant::now(),
        }
    }

    pub fn register_latency(&mut self, current_latency: f32) {
        let now = Instant::now();
        let dt = (now - self.last_time).as_secs_f32();
        self.last_time = now;

        let is_good = current_latency < self.max_latency_sec;

        if is_good && self.accept_rate == 1.0 {
            return; // early out
        }

        /// If we let it go too low, we won't accept any messages,
        /// and then we won't ever recover.
        const MIN_ACCEPT_RATE: f32 = 0.01;

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
            self.accept_rate -= badness * dt / 5.0;
        }

        self.accept_rate = self.accept_rate.clamp(MIN_ACCEPT_RATE, 1.0);

        if self.last_log_time.elapsed().as_secs_f32() > 1.0 {
            re_log::debug!(
                "Currently dropping {:.2}% of messages to keep latency low",
                100.0 * (1.0 - self.accept_rate)
            );
            self.last_log_time = Instant::now();
        }
    }
}
