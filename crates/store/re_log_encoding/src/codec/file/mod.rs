mod headers;

pub use self::headers::{EncodingOptions, FileHeader, MessageHeader, MessageKind, OptionsError};

#[cfg(feature = "decoder")]
pub mod decoder;

#[cfg(feature = "encoder")]
pub mod encoder;
