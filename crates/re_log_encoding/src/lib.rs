//! Crate that handles encoding of rerun log types.

#[cfg(feature = "decoder")]
pub mod decoder;
#[cfg(feature = "encoder")]
#[cfg(not(target_arch = "wasm32"))] // we do no yet support encoding LogMsgs in the browser
pub mod encoder;

#[cfg(feature = "encoder")]
#[cfg(not(target_arch = "wasm32"))]
mod file_sink;

#[cfg(feature = "decoder")]
pub mod stream_rrd_from_http;

// ---------------------------------------------------------------------

#[cfg(feature = "encoder")]
#[cfg(not(target_arch = "wasm32"))]
pub use file_sink::{FileSink, FileSinkError};

// ----------------------------------------------------------------------------

#[cfg(any(feature = "encoder", feature = "decoder"))]
const RRD_HEADER: &[u8; 4] = b"RRF1";

// ----------------------------------------------------------------------------

/// Compression format used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Compression {
    Off = 0,

    /// Very fast compression and decompression, but not very good compression ratio.
    LZ4 = 1,
}

/// How we serialize the data
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Serializer {
    MsgPack = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncodingOptions {
    pub compression: Compression,
    pub serializer: Serializer,
}

impl EncodingOptions {
    pub const UNCOMPRESSED: Self = Self {
        compression: Compression::Off,
        serializer: Serializer::MsgPack,
    };
    pub const COMPRESSED: Self = Self {
        compression: Compression::LZ4,
        serializer: Serializer::MsgPack,
    };

    pub fn from_bytes(bytes: [u8; 4]) -> Result<Self, OptionsError> {
        match bytes {
            [compression, serializer, 0, 0] => {
                let compression = match compression {
                    0 => Compression::Off,
                    1 => Compression::LZ4,
                    _ => return Err(OptionsError::UnknownCompression(compression)),
                };
                let serializer = match serializer {
                    1 => Serializer::MsgPack,
                    _ => return Err(OptionsError::UnknownSerializer(serializer)),
                };
                Ok(Self {
                    compression,
                    serializer,
                })
            }
            _ => Err(OptionsError::UnknownReservedBytes),
        }
    }

    pub fn to_bytes(&self) -> [u8; 4] {
        [
            self.compression as u8,
            self.serializer as u8,
            0, // reserved
            0, // reserved
        ]
    }
}

/// On failure to decode [`EncodingOptions`]
#[derive(thiserror::Error, Debug)]
pub enum OptionsError {
    #[error("Reserved bytes not zero")]
    UnknownReservedBytes,

    #[error("Unknown compression: {0}")]
    UnknownCompression(u8),

    #[error("Unknown serializer: {0}")]
    UnknownSerializer(u8),
}

// ---------------------------------------------------------------------------

/// Profiling macro for feature "puffin"
#[doc(hidden)]
#[macro_export]
macro_rules! profile_function {
    ($($arg: tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        puffin::profile_function!($($arg)*);
    };
}

/// Profiling macro for feature "puffin"
#[doc(hidden)]
#[macro_export]
macro_rules! profile_scope {
    ($($arg: tt)*) => {
        #[cfg(not(target_arch = "wasm32"))]
        puffin::profile_scope!($($arg)*);
    };
}
