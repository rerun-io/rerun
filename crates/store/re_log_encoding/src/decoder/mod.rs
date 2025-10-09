//! Decoding [`LogMsg`]:es from `.rrd` files/streams.

mod errors;
mod helpers;

pub use self::{
    errors::{DecodeError, NotAnRrdError},
    helpers::options_from_bytes,
};

pub mod stream;

#[cfg(feature = "decoder")]
pub mod streaming;
