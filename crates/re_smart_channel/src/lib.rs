//! A channel that keeps track of latency and queue length.

use std::sync::{
    atomic::{AtomicU64, Ordering::Relaxed},
    Arc,
};

use crossbeam::channel::{RecvError, SendError, TryRecvError};
use instant::Instant;

pub fn smart_channel<T: Send>() -> (SmartSender<T>, SmartReceiver<T>) {
    let (tx, rx) = crossbeam::channel::unbounded();
    let stats = Arc::new(SharedStats::default());
    let sender = SmartSender {
        tx,
        stats: stats.clone(),
    };
    let receiver = SmartReceiver { rx, stats };
    (sender, receiver)
}

#[derive(Default)]
struct SharedStats {
    /// Latest known latency from sending a message to receiving it, it nanoseconds.
    latency_ns: AtomicU64,
}

pub struct SmartSender<T: Send> {
    tx: crossbeam::channel::Sender<(Instant, T)>,
    stats: Arc<SharedStats>,
}

impl<T: Send> SmartSender<T> {
    pub fn send(&self, msg: T) -> Result<(), SendError<T>> {
        self.tx
            .send((Instant::now(), msg))
            .map_err(|SendError((_, msg))| SendError(msg))
    }

    /// Number of messages in the channel right now.
    pub fn queue_len(&self) -> usize {
        self.tx.len()
    }

    /// Latest known latency from sending a message to receiving it, it nanoseconds.
    pub fn latency_ns(&self) -> u64 {
        self.stats.latency_ns.load(Relaxed)
    }
}

pub struct SmartReceiver<T: Send> {
    rx: crossbeam::channel::Receiver<(Instant, T)>,
    stats: Arc<SharedStats>,
}

impl<T: Send> SmartReceiver<T> {
    pub fn recv(&self) -> Result<T, RecvError> {
        let (sent, msg) = self.rx.recv()?;
        let latency_ns = sent.elapsed().as_nanos() as u64;
        self.stats.latency_ns.store(latency_ns, Relaxed);
        Ok(msg)
    }

    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        let (sent, msg) = self.rx.try_recv()?;
        let latency_ns = sent.elapsed().as_nanos() as u64;
        self.stats.latency_ns.store(latency_ns, Relaxed);
        Ok(msg)
    }

    /// Number of messages in the channel right now.
    pub fn queue_len(&self) -> usize {
        self.rx.len()
    }

    /// Latest known latency from sending a message to receiving it, it nanoseconds.
    pub fn latency_ns(&self) -> u64 {
        self.stats.latency_ns.load(Relaxed)
    }
}

#[test]
fn test_smart_channel() {
    let (tx, rx) = smart_channel();

    assert_eq!(tx.queue_len(), 0);
    assert_eq!(rx.queue_len(), 0);
    assert_eq!(tx.latency_ns(), 0);

    tx.send(42).unwrap();

    assert_eq!(tx.queue_len(), 1);
    assert_eq!(rx.queue_len(), 1);
    assert_eq!(tx.latency_ns(), 0);

    std::thread::sleep(std::time::Duration::from_millis(10));

    assert_eq!(rx.recv(), Ok(42));

    assert_eq!(tx.queue_len(), 0);
    assert_eq!(rx.queue_len(), 0);
    assert!(tx.latency_ns() > 1_000_000);
}
