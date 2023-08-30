use std::sync::{
    atomic::{AtomicBool, Ordering::Relaxed},
    Arc,
};

use crate::{
    RecvError, RecvTimeoutError, SharedStats, SmartChannelSource, SmartMessage, TryRecvError,
};

pub struct Receiver<T: Send> {
    pub(crate) rx: crossbeam::channel::Receiver<SmartMessage<T>>,
    stats: Arc<SharedStats>,
    pub(crate) source: Arc<SmartChannelSource>,
    connected: AtomicBool,
}

impl<T: Send> Receiver<T> {
    pub(crate) fn new(
        rx: crossbeam::channel::Receiver<SmartMessage<T>>,
        stats: Arc<SharedStats>,
        source: Arc<SmartChannelSource>,
    ) -> Self {
        Self {
            rx,
            stats,
            source,
            connected: AtomicBool::new(true),
        }
    }

    /// Are we still connected?
    ///
    /// Once false, we will never be connected again: the source has run dry.
    ///
    /// This is only updated once one of the receive methods fails.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Relaxed)
    }

    pub fn recv(&self) -> Result<SmartMessage<T>, RecvError> {
        let msg = match self.rx.recv() {
            Ok(x) => x,
            Err(RecvError) => {
                self.connected.store(false, Relaxed);
                return Err(RecvError);
            }
        };

        let latency_ns = msg.time.elapsed().as_nanos() as u64;
        self.stats.latency_ns.store(latency_ns, Relaxed);

        Ok(msg)
    }

    pub fn try_recv(&self) -> Result<SmartMessage<T>, TryRecvError> {
        let msg = match self.rx.try_recv() {
            Ok(x) => x,
            Err(err) => {
                if err == TryRecvError::Disconnected {
                    self.connected.store(false, Relaxed);
                }
                return Err(err);
            }
        };

        let latency_ns = msg.time.elapsed().as_nanos() as u64;
        self.stats.latency_ns.store(latency_ns, Relaxed);

        Ok(msg)
    }

    pub fn recv_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> Result<SmartMessage<T>, RecvTimeoutError> {
        let msg = match self.rx.recv_timeout(timeout) {
            Ok(x) => x,
            Err(err) => {
                if err == RecvTimeoutError::Disconnected {
                    self.connected.store(false, Relaxed);
                }
                return Err(err);
            }
        };

        let latency_ns = msg.time.elapsed().as_nanos() as u64;
        self.stats.latency_ns.store(latency_ns, Relaxed);

        Ok(msg)
    }

    /// Receives without registering the latency.
    ///
    /// This is for use with [`crate::Sender::send_at`] when chaining to another channel
    /// created with [`Self::chained_channel`].
    pub fn recv_with_send_time(&self) -> Result<SmartMessage<T>, RecvError> {
        self.rx.recv()
    }

    /// Where is the data coming from?
    #[inline]
    pub fn source(&self) -> &SmartChannelSource {
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
    /// Care must be taken to use [`Self::recv_with_send_time`] and [`crate::Sender::send_at`].
    /// This is a very leaky abstraction, and it would be nice with a refactor.
    pub fn chained_channel(&self) -> (crate::Sender<T>, Receiver<T>) {
        crate::smart_channel_with_stats(
            // NOTE: We cannot know yet, and it doesn't matter as the new sender will only be used
            // to forward existing messages.
            crate::SmartMessageSource::Unknown,
            self.source.clone(),
            self.stats.clone(),
        )
    }
}
