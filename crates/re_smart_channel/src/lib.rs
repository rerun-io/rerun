//! A channel that keeps track of latency and queue length.

use std::sync::{
    atomic::{AtomicU64, Ordering::Relaxed},
    Arc,
};

use crossbeam::channel::{RecvError, SendError, TryRecvError};
use instant::Instant;

/// Where is the messages coming from?
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    Network,
    File,
}

pub fn smart_channel<T: Send>(source: Source) -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = crossbeam::channel::unbounded();
    let stats = Arc::new(SharedStats::default());
    let sender = Sender {
        tx,
        stats: stats.clone(),
    };
    let receiver = Receiver { rx, stats, source };
    (sender, receiver)
}

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
}

pub struct Receiver<T: Send> {
    rx: crossbeam::channel::Receiver<(Instant, T)>,
    stats: Arc<SharedStats>,
    source: Source,
}

impl<T: Send> Receiver<T> {
    pub fn recv(&self) -> Result<T, RecvError> {
        Ok(self.recv_with_send_time()?.1)
    }

    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        let (sent, msg) = self.rx.try_recv()?;
        let latency_ns = sent.elapsed().as_nanos() as u64;
        self.stats.latency_ns.store(latency_ns, Relaxed);
        Ok(msg)
    }

    pub fn recv_with_send_time(&self) -> Result<(Instant, T), RecvError> {
        let (sent, msg) = self.rx.recv()?;
        let latency_ns = sent.elapsed().as_nanos() as u64;
        self.stats.latency_ns.store(latency_ns, Relaxed);
        Ok((sent, msg))
    }

    /// Where is the data coming from?
    #[inline]
    pub fn source(&self) -> Source {
        self.source
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
}

#[test]
fn test_smart_channel() {
    let (tx, rx) = smart_channel(Source::Network);

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
