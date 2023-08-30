use std::sync::{atomic::Ordering::Relaxed, Arc};

use web_time::Instant;

use crate::{SendError, SharedStats, SmartMessage, SmartMessagePayload, SmartMessageSource};

#[derive(Clone)]
pub struct Sender<T: Send> {
    tx: crossbeam::channel::Sender<SmartMessage<T>>,
    source: Arc<SmartMessageSource>,
    stats: Arc<SharedStats>,
}

impl<T: Send> Sender<T> {
    pub(crate) fn new(
        tx: crossbeam::channel::Sender<SmartMessage<T>>,
        source: Arc<SmartMessageSource>,
        stats: Arc<SharedStats>,
    ) -> Self {
        Self { tx, source, stats }
    }

    /// Clones the sender with an updated source.
    pub fn clone_as(&self, source: SmartMessageSource) -> Self {
        Self {
            tx: self.tx.clone(),
            source: Arc::new(source),
            stats: Arc::clone(&self.stats),
        }
    }

    pub fn send(&self, msg: T) -> Result<(), SendError<T>> {
        self.send_at(
            Instant::now(),
            Arc::clone(&self.source),
            SmartMessagePayload::Msg(msg),
        )
    }

    /// Forwards a message as-is.
    pub fn send_at(
        &self,
        time: Instant,
        source: Arc<SmartMessageSource>,
        payload: SmartMessagePayload<T>,
    ) -> Result<(), SendError<T>> {
        // NOTE: We should never be sending a message with an unknown source.
        debug_assert!(!matches!(*source, SmartMessageSource::Unknown));

        self.tx
            .send(SmartMessage {
                time,
                source,
                payload,
            })
            .map_err(|SendError(msg)| SendError(msg.into_data().unwrap()))
    }

    /// Used to indicate that a sender has left.
    ///
    /// This sends a message down the channel allowing the receiving end to know whether one of the
    /// sender has left, and if so why (if applicable).
    ///
    /// Using a [`Sender`] after calling `quit` is undefined behavior: the receiving end is free
    /// to silently drop those messages (or worse).
    pub fn quit(
        &self,
        err: Option<Box<dyn std::error::Error + Send>>,
    ) -> Result<(), SendError<SmartMessage<T>>> {
        // NOTE: We should never be sending a message with an unknown source.
        debug_assert!(!matches!(*self.source, SmartMessageSource::Unknown));

        self.tx.send(SmartMessage {
            time: Instant::now(),
            source: Arc::clone(&self.source),
            payload: SmartMessagePayload::Quit(err),
        })
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
