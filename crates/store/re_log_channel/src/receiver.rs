use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::Relaxed;

use crate::{Channel, LoadCommand, LogSource, SmartMessage, TryRecvError};

pub struct LogReceiver {
    rx: crossbeam::channel::Receiver<SmartMessage>,
    tx: async_channel::Sender<LoadCommand>,
    channel: Arc<Channel>,
    source: Arc<LogSource>,
    connected: AtomicBool,
}

impl LogReceiver {
    pub(crate) fn new(
        rx: crossbeam::channel::Receiver<SmartMessage>,
        tx: async_channel::Sender<LoadCommand>,
        channel: Arc<Channel>,
        source: Arc<LogSource>,
    ) -> Self {
        Self {
            rx,
            tx,
            channel,
            source,
            connected: AtomicBool::new(true),
        }
    }

    /// Call this on each sent message.
    ///
    /// Can be used to wake up the receiver thread.
    pub fn set_waker(&self, waker: impl Fn() + Send + Sync + 'static) {
        *self.channel.waker.write() = Some(Box::new(waker));
    }

    /// Are we still connected?
    ///
    /// Once false, we will never be connected again: the source has run dry.
    ///
    /// This is only updated once one of the receive methods fails.
    pub fn is_connected(&self) -> bool {
        self.connected.load(Relaxed)
    }

    #[cfg(not(target_arch = "wasm32"))] // Cannot block on web
    pub fn recv(&self) -> Result<SmartMessage, crate::RecvError> {
        let Ok(msg) = self.rx.recv() else {
            self.connected.store(false, Relaxed);
            return Err(crate::RecvError);
        };

        Ok(msg)
    }

    pub fn try_recv(&self) -> Result<SmartMessage, TryRecvError> {
        let msg = match self.rx.try_recv() {
            Ok(x) => x,
            Err(err) => {
                if err == TryRecvError::Disconnected {
                    self.connected.store(false, Relaxed);
                }
                return Err(err);
            }
        };

        Ok(msg)
    }

    #[cfg(not(target_arch = "wasm32"))] // Cannot block on web
    pub fn recv_timeout(
        &self,
        timeout: std::time::Duration,
    ) -> Result<SmartMessage, crate::RecvTimeoutError> {
        let msg = match self.rx.recv_timeout(timeout) {
            Ok(x) => x,
            Err(err) => {
                if err == crate::RecvTimeoutError::Disconnected {
                    self.connected.store(false, Relaxed);
                }
                return Err(err);
            }
        };

        Ok(msg)
    }

    pub(crate) fn inner(&self) -> &crossbeam::channel::Receiver<SmartMessage> {
        &self.rx
    }

    /// Where is the data coming from?
    #[inline]
    pub fn source(&self) -> &LogSource {
        &self.source
    }

    /// Where is the data coming from?
    #[inline]
    pub fn source_arc(&self) -> Arc<LogSource> {
        self.source.clone()
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

    /// Returns true if there is a [`crate::LogSender`] that is waiting
    /// to receive commands to load chunks.
    ///
    /// Can be used to debounce load requests.
    pub fn has_waiting_command_receivers(&self) -> bool {
        0 < self.channel.num_waiting_receivers.load(Relaxed)
    }

    /// Send a command to the other end.
    ///
    /// You should probably check `has_waiting_command_receivers` before using this method (debounce).
    pub fn send_command(&self, rb: LoadCommand) {
        let tx = self.tx.clone();
        spawn_future(async move {
            tx.send(rb).await.ok();
        });
    }
}

#[cfg(target_arch = "wasm32")]
fn spawn_future<F>(future: F)
where
    F: std::future::Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(future);
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_future<F>(future: F)
where
    F: std::future::Future<Output = ()> + 'static + Send,
{
    tokio::spawn(future);
}
