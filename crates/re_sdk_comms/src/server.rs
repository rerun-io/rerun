//! TODO(emilk): use tokio instead

use std::time::Instant;

use anyhow::Context as _;
use rand::{Rng as _, SeedableRng};

use re_log_types::{LogMsg, TimePoint, TimeType, TimelineName};
use re_smart_channel::{Receiver, Sender};

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
/// let log_msg_rx = serve(80, ServerOptions::default())?;
/// # Ok::<(), anyhow::Error>(())
/// ```
pub fn serve(port: u16, options: ServerOptions) -> anyhow::Result<Receiver<LogMsg>> {
    let bind_addr = format!("0.0.0.0:{port}");

    let listener = std::net::TcpListener::bind(&bind_addr)
        .with_context(|| format!("Failed to bind address {bind_addr:?}"))?;

    let (tx, rx) = re_smart_channel::smart_channel(re_smart_channel::Source::TcpServer { port });

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
                        re_log::warn!("Failed to accept incoming SDK client: {err}");
                    }
                }
            }
        })
        .expect("Failed to spawn thread");

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

fn spawn_client(stream: std::net::TcpStream, tx: Sender<LogMsg>, options: ServerOptions) {
    std::thread::Builder::new()
        .name(format!(
            "sdk-server-client-handler-{:?}",
            stream.peer_addr()
        ))
        .spawn(move || {
            if options.quiet {
                re_log::debug!("New SDK client connected: {:?}", stream.peer_addr());
            } else {
                re_log::info!("New SDK client connected: {:?}", stream.peer_addr());
            }

            if let Err(err) = run_client(stream, &tx, options) {
                re_log::warn!("Closing connection to client: {err}");
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

        congestion_manager.register_latency(tx.latency_sec());

        let msg = crate::decode_log_msg(&packet)?;

        if matches!(msg, LogMsg::Goodbye(_)) {
            re_log::debug!("Client sent goodbye message.");
            return Ok(());
        }

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
            LogMsg::BeginRecordingMsg(_) | LogMsg::EntityPathOpMsg(_) | LogMsg::Goodbye(_) => true, // we don't want to drop any of these

            LogMsg::ArrowMsg(arrow_msg) => match arrow_msg.time_point() {
                Ok(time_point) => self.should_send_time_point(&time_point),
                Err(err) => {
                    re_log::error_once!("Failed to parse an Arrow Message - dropping this message, and maybe more. {err}");
                    false
                }
            },
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
                    let oldest_time = *timeline_history.send_time.keys().next().unwrap();
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
