//! A mpsc channel that applies backpressure based on byte size.

mod try_send_error;

pub mod sync;

pub use self::sync::{
    Receiver, RecvError, RecvTimeoutError, Select, SelectTimeoutError, SelectedOperation,
    SendError, Sender, TryRecvError, TrySelectError, channel,
};

/// A message together with its size in bytes.
pub struct SizedMessage<T> {
    pub msg: T,
    pub size_bytes: u64,
}
