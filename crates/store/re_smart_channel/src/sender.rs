use std::sync::Arc;

use crate::{Channel, SendError, SmartMessage, SmartMessagePayload, SmartMessageSource};

#[derive(Clone)]
pub struct Sender<T: Send> {
    tx: crossbeam::channel::Sender<SmartMessage<T>>,
    source: Arc<SmartMessageSource>,
    channel: Arc<Channel>,
}

impl<T: Send> Sender<T> {
    pub(crate) fn new(
        tx: crossbeam::channel::Sender<SmartMessage<T>>,
        source: Arc<SmartMessageSource>,
        channel: Arc<Channel>,
    ) -> Self {
        Self {
            tx,
            source,
            channel,
        }
    }

    /// Send a message to the receiver
    pub fn send(&self, msg: T) -> Result<(), SendError<T>> {
        let source = Arc::clone(&self.source);

        // NOTE: We should never be sending a message with an unknown source.
        debug_assert!(!matches!(*source, SmartMessageSource::Unknown));

        let payload = SmartMessagePayload::Msg(msg);

        self.tx
            .send(SmartMessage { source, payload })
            .map_err(|SendError(msg)| SendError(msg.payload))
            .map_err(|SendError(msg)| match msg {
                SmartMessagePayload::Msg(msg) => SendError(msg),
                SmartMessagePayload::Flush { .. } | SmartMessagePayload::Quit(_) => unreachable!(),
            })?;

        if let Some(waker) = self.channel.waker.read().as_ref() {
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

        if let Some(waker) = self.channel.waker.read().as_ref() {
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
            source: Arc::clone(&self.source),
            payload: SmartMessagePayload::Quit(err),
        })?;

        if let Some(waker) = self.channel.waker.read().as_ref() {
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
