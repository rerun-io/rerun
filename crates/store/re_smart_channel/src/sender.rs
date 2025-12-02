use std::sync::Arc;

use web_time::Instant;

use crate::{Channel, SendError, SmartMessage, SmartMessagePayload, SmartMessageSource};

#[derive(Clone)]
pub struct Sender<T: Send> {
    tx: crossbeam::channel::Sender<SmartMessage<T>>,
    source: Arc<SmartMessageSource>,
    stats: Arc<Channel>,
}

impl<T: Send> Sender<T> {
    pub(crate) fn new(
        tx: crossbeam::channel::Sender<SmartMessage<T>>,
        source: Arc<SmartMessageSource>,
        stats: Arc<Channel>,
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
        .map_err(|SendError(msg)| match msg {
            SmartMessagePayload::Msg(msg) => SendError(msg),
            SmartMessagePayload::Flush { .. } | SmartMessagePayload::Quit(_) => unreachable!(),
        })
    }

    /// Forwards a message as-is.
    fn send_at(
        &self,
        time: Instant,
        source: Arc<SmartMessageSource>,
        payload: SmartMessagePayload<T>,
    ) -> Result<(), SendError<SmartMessagePayload<T>>> {
        // NOTE: We should never be sending a message with an unknown source.
        debug_assert!(!matches!(*source, SmartMessageSource::Unknown));

        self.tx
            .send(SmartMessage {
                time,
                source,
                payload,
            })
            .map_err(|SendError(msg)| SendError(msg.payload))?;

        if let Some(waker) = self.stats.waker.read().as_ref() {
            (waker)();
        }

        Ok(())
    }

    /// Blocks until all previously sent messages have been received.
    ///
    /// Note: This is only implemented for non-wasm targets since we cannot make
    /// blocking calls on web.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn flush_blocking(&self, timeout: std::time::Duration) -> Result<(), crate::FlushError> {
        use crate::FlushError;

        let (tx, rx) = std::sync::mpsc::sync_channel(0); // oneshot
        self.tx
            .send(SmartMessage {
                time: Instant::now(),
                source: Arc::clone(&self.source),
                payload: SmartMessagePayload::Flush {
                    on_flush_done: Box::new(move || {
                        tx.send(()).ok();
                    }),
                },
            })
            .map_err(|_ignored| FlushError::Closed)?;

        rx.recv_timeout(timeout).map_err(|err| match err {
            std::sync::mpsc::RecvTimeoutError::Timeout => FlushError::Timeout,
            std::sync::mpsc::RecvTimeoutError::Disconnected => FlushError::Closed,
        })?;

        if let Some(waker) = self.stats.waker.read().as_ref() {
            (waker)();
        }

        Ok(())
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
        })?;

        if let Some(waker) = self.stats.waker.read().as_ref() {
            (waker)();
        }

        Ok(())
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
}
