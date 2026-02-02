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
