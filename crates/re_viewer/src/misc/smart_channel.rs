use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering::Relaxed},
    mpsc::{RecvError, SendError, TryRecvError},
    Arc,
};

use instant::Instant;

pub fn smart_channel<T: Send>() -> (SmartSender<T>, SmartReceiver<T>) {
    let (tx, rx) = std::sync::mpsc::channel();
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
    /// Number of elements in the channel right now.
    len: AtomicUsize,

    /// Latest known latency from sending a message to receiving it, it nanoseconds.
    latency_ns: AtomicU64,
}

pub struct SmartSender<T: Send> {
    tx: std::sync::mpsc::Sender<(Instant, T)>,
    stats: Arc<SharedStats>,
}

impl<T: Send> SmartSender<T> {
    pub fn send(&self, msg: T) -> Result<(), SendError<T>> {
        self.tx
            .send((Instant::now(), msg))
            .map_err(|SendError((_, msg))| SendError(msg))?;
        self.stats.len.fetch_add(1, Relaxed);
        Ok(())
    }

    /// Number of elements in the channel right now.
    pub fn queue_len(&self) -> usize {
        self.stats.len.load(Relaxed)
    }

    /// Latest known latency from sending a message to receiving it, it nanoseconds.
    pub fn latency_ns(&self) -> u64 {
        self.stats.latency_ns.load(Relaxed)
    }
}

pub struct SmartReceiver<T: Send> {
    rx: std::sync::mpsc::Receiver<(Instant, T)>,
    stats: Arc<SharedStats>,
}

impl<T: Send> SmartReceiver<T> {
    pub fn recv(&self) -> Result<T, RecvError> {
        let (sent, msg) = self.rx.recv()?;
        self.stats.len.fetch_sub(1, Relaxed);
        let latency_ns = sent.elapsed().as_nanos() as u64;
        self.stats.latency_ns.store(latency_ns, Relaxed);
        Ok(msg)
    }

    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        let (sent, msg) = self.rx.try_recv()?;
        self.stats.len.fetch_sub(1, Relaxed);
        let latency_ns = sent.elapsed().as_nanos() as u64;
        self.stats.latency_ns.store(latency_ns, Relaxed);
        Ok(msg)
    }

    /// Number of elements in the channel right now.
    pub fn queue_len(&self) -> usize {
        self.stats.len.load(Relaxed)
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
