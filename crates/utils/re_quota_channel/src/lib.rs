//! A mpsc channel that applies backpressure based on byte size.

mod try_send_error;

#[cfg(not(target_arch = "wasm32"))]
pub mod r#async;

pub mod sync;

#[cfg(not(target_arch = "wasm32"))]
pub use self::r#async::{AsyncReceiver, AsyncSender, async_channel};

pub use self::sync::{
    Receiver, RecvError, RecvTimeoutError, Select, SelectTimeoutError, SelectedOperation,
    SendError, Sender, TryRecvError, TrySelectError, channel,
};

pub use self::try_send_error::TrySendError;

/// A message together with its size in bytes.
pub struct SizedMessage<T> {
    pub msg: T,
    pub size_bytes: u64,
}
