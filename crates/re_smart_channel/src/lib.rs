//! A channel that keeps track of latency and queue length.

use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering::Relaxed},
    Arc,
};

use instant::Instant;

pub use crossbeam::channel::{RecvError, RecvTimeoutError, SendError, TryRecvError};

/// Where is the messages coming from?
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    /// The source if a file on disk
    File { path: std::path::PathBuf },

    /// Streaming an `.rrd` file over http.
    RrdHttpStream { url: String },

    /// Loading an `.rrd` file from a `postMessage` js event
    ///
    /// Only applicable to web browser iframes
    RrdWebEventListener,

    /// The source is the logging sdk directly, same process.
    Sdk,

    /// We are a WebSocket client connected to a rerun server.
    ///
    /// We are likely running in a web browser.
    WsClient {
        /// The server we are connected to (or are trying to connect to)
        ws_server_url: String,
    },

    /// We are a TCP server listening on this port
    TcpServer { port: u16 },
}

impl Source {
    pub fn is_network(&self) -> bool {
        match self {
            Self::File { .. } | Self::Sdk | Self::RrdWebEventListener => false,
            Self::RrdHttpStream { .. } | Self::WsClient { .. } | Self::TcpServer { .. } => true,
        }
    }
}

pub fn smart_channel<T: Send>(source: Source) -> (Sender<T>, Receiver<T>) {
    let stats = Arc::new(SharedStats::default());
    smart_channel_with_stats(source, stats)
}

/// Create a new channel using the same stats as some other.
///
/// This is a very leaky abstraction, and it would be nice to refactor some day
fn smart_channel_with_stats<T: Send>(
    source: Source,
    stats: Arc<SharedStats>,
) -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = crossbeam::channel::unbounded();
    let sender = Sender {
        tx,
        stats: stats.clone(),
    };
    let receiver = Receiver {
        rx,
        stats,
        source,
        connected: AtomicBool::new(true),
    };
    (sender, receiver)
}

/// Stats for a channel, possibly shared between chained channels.
#[derive(Default)]
struct SharedStats {
    /// Latest known latency from sending a message to receiving it, it nanoseconds.
    latency_ns: AtomicU64,
}

#[derive(Clone)]
pub struct Sender<T: Send> {
    tx: crossbeam::channel::Sender<(Instant, T)>,
    stats: Arc<SharedStats>,
}

impl<T: Send> Sender<T> {
    pub fn send(&self, msg: T) -> Result<(), SendError<T>> {
        self.send_at(Instant::now(), msg)
    }

    /// back-date a message
    pub fn send_at(&self, time: Instant, msg: T) -> Result<(), SendError<T>> {
        self.tx
            .send((time, msg))
            .map_err(|SendError((_, msg))| SendError(msg))
    }

    /// Is the channel currently empty of messages?
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.tx.is_empty()
    }

    /// Number of messages in the channel right now.
    #[inline]
    pub fn len(&self) -> usize {
        self.tx.len()
    }

    /// Latest known latency from sending a message to receiving it, it nanoseconds.
    pub fn latency_ns(&self) -> u64 {
        self.stats.latency_ns.load(Relaxed)
    }

    /// Latest known latency from sending a message to receiving it,
    /// in seconds
    pub fn latency_sec(&self) -> f32 {
        self.latency_ns() as f32 / 1e9
    }
}

pub struct Receiver<T: Send> {
    rx: crossbeam::channel::Receiver<(Instant, T)>,
    stats: Arc<SharedStats>,
    source: Source,
    connected: AtomicBool,
}

impl<T: Send> Receiver<T> {
    /// Are we still connected?
    ///
    /// Once false, we will never be connected again: the source has run dry.
    ///
    /// This is only updated once one of the receive methods fails.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Relaxed)
    }

    pub fn recv(&self) -> Result<T, RecvError> {
        let (sent, msg) = match self.rx.recv() {
            Ok(x) => x,
            Err(RecvError) => {
                self.connected.store(false, Relaxed);
                return Err(RecvError);
            }
        };
        let latency_ns = sent.elapsed().as_nanos() as u64;
        self.stats.latency_ns.store(latency_ns, Relaxed);
        Ok(msg)
    }

    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        let (sent, msg) = match self.rx.try_recv() {
            Ok(x) => x,
            Err(err) => {
                if err == TryRecvError::Disconnected {
                    self.connected.store(false, Relaxed);
                }
                return Err(err);
            }
        };
        let latency_ns = sent.elapsed().as_nanos() as u64;
        self.stats.latency_ns.store(latency_ns, Relaxed);
        Ok(msg)
    }

    pub fn recv_timeout(&self, timeout: std::time::Duration) -> Result<T, RecvTimeoutError> {
        let (sent, msg) = match self.rx.recv_timeout(timeout) {
            Ok(x) => x,
            Err(err) => {
                if err == RecvTimeoutError::Disconnected {
                    self.connected.store(false, Relaxed);
                }
                return Err(err);
            }
        };
        let latency_ns = sent.elapsed().as_nanos() as u64;
        self.stats.latency_ns.store(latency_ns, Relaxed);
        Ok(msg)
    }

    /// Receives without registering the latency.
    ///
    /// This is for use with [`Sender::send_at`] when chaining to another channel
    /// created with [`Self::chained_channel`].
    pub fn recv_with_send_time(&self) -> Result<(Instant, T), RecvError> {
        self.rx.recv()
    }

    /// Where is the data coming from?
    #[inline]
    pub fn source(&self) -> &Source {
        &self.source
    }

    /// Is the channel currently empty of messages?
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.rx.is_empty()
    }

    /// Number of messages in the channel right now.
    #[inline]
    pub fn len(&self) -> usize {
        self.rx.len()
    }

    /// Latest known latency from sending a message to receiving it, it nanoseconds.
    pub fn latency_ns(&self) -> u64 {
        self.stats.latency_ns.load(Relaxed)
    }

    /// Latest known latency from sending a message to receiving it,
    /// in seconds
    pub fn latency_sec(&self) -> f32 {
        self.latency_ns() as f32 / 1e9
    }

    /// Create a new channel that use the same stats as this one.
    ///
    /// This means both channels will see the same latency numbers.
    ///
    /// Care must be taken to use [`Self::recv_with_send_time`] and [`Sender::send_at`].
    /// This is a very leaky abstraction, and it would be nice with a refactor.
    pub fn chained_channel(&self) -> (Sender<T>, Receiver<T>) {
        smart_channel_with_stats(self.source.clone(), self.stats.clone())
    }
}

#[test]
fn test_smart_channel() {
    let (tx, rx) = smart_channel(Source::Sdk); // whatever source

    assert_eq!(tx.len(), 0);
    assert_eq!(rx.len(), 0);
    assert_eq!(tx.latency_ns(), 0);

    tx.send(42).unwrap();

    assert_eq!(tx.len(), 1);
    assert_eq!(rx.len(), 1);
    assert_eq!(tx.latency_ns(), 0);

    std::thread::sleep(std::time::Duration::from_millis(10));

    assert_eq!(rx.recv(), Ok(42));

    assert_eq!(tx.len(), 0);
    assert_eq!(rx.len(), 0);
    assert!(tx.latency_ns() > 1_000_000);
}

#[test]
fn test_smart_channel_connected() {
    let (tx1, rx) = smart_channel(Source::Sdk); // whatever source
    assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
    assert!(rx.is_connected());

    let tx2 = tx1.clone();
    assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
    assert!(rx.is_connected());

    tx2.send(42).unwrap();
    assert_eq!(rx.try_recv(), Ok(42));
    assert!(rx.is_connected());

    drop(tx1);
    assert_eq!(rx.try_recv(), Err(TryRecvError::Empty));
    assert!(rx.is_connected());

    drop(tx2);
    assert_eq!(rx.try_recv(), Err(TryRecvError::Disconnected));
    assert!(!rx.is_connected());
}
