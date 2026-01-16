//! A mpsc channel that applies backpressure based on byte size.
//!
//! This crate provides both synchronous channel implementations
//! that apply backpressure based on the total byte size of messages in the channel.

pub mod sync;

pub use sync::{
    Receiver, RecvError, RecvTimeoutError, Select, SelectTimeoutError, SelectedOperation,
    SendError, Sender, SizedMessage, TryRecvError, TrySelectError, TrySendError, channel,
};
