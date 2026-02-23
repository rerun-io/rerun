//! A mpsc channel that applies backpressure based on byte size.

/// Same as [`BLOCKED_WARNING_THRESHOLD`] but as a raw `u64` for use in format strings.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const BLOCKED_WARNING_THRESHOLD_SECS: u64 = 5;

/// How long to wait before logging a warning that send is blocked.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const BLOCKED_WARNING_THRESHOLD: std::time::Duration =
    std::time::Duration::from_secs(BLOCKED_WARNING_THRESHOLD_SECS);

mod try_send_error;

pub mod sync;

/// Async (tokio-based) broadcast channel with byte-based backpressure.
///
/// Only available on native platforms with the `tokio` feature enabled.
#[cfg(all(not(target_arch = "wasm32"), feature = "tokio"))]
pub mod async_broadcast_channel;

/// Thin wrapper around [`tokio::sync::mpsc`] that logs a warning if send blocks too long.
///
/// Only available on native platforms with the `tokio` feature enabled.
#[cfg(all(not(target_arch = "wasm32"), feature = "tokio"))]
pub mod async_mpsc_channel;

pub use sync::{
    Receiver, RecvError, RecvTimeoutError, Select, SelectTimeoutError, SelectedOperation,
    SendError, Sender, TryRecvError, TrySelectError, channel,
};

/// A message together with its size in bytes.
pub struct SizedMessage<T> {
    pub msg: T,
    pub size_bytes: u64,
}

/// Send a message on a crossbeam channel, and warn if it is taking too long.
#[track_caller]
pub fn send_crossbeam<T: std::fmt::Debug>(
    sender: &crossbeam::channel::Sender<T>,
    msg: T,
) -> Result<(), crossbeam::channel::SendError<T>> {
    #[cfg(target_arch = "wasm32")]
    {
        // On web we cannot block, so we just do a normal send.
        sender.send(msg)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        use crossbeam::channel::SendTimeoutError;

        match sender.send_timeout(msg, BLOCKED_WARNING_THRESHOLD) {
            Ok(()) => Ok(()),
            Err(SendTimeoutError::Disconnected(msg)) => Err(crossbeam::channel::SendError(msg)),
            Err(SendTimeoutError::Timeout(msg)) => {
                let caller = std::panic::Location::caller();
                re_log::debug_once!(
                    "{}:{}: failed to send message within {BLOCKED_WARNING_THRESHOLD_SECS}s. Message: {msg:?}. Will keep blocking…",
                    caller.file(),
                    caller.line(),
                );

                #[expect(clippy::disallowed_methods)]
                // This is the one place we're allowed to call `Sender::send`.
                sender.send(msg)
            }
        }
    }
}
