//! Video frame decoding.

#[cfg(feature = "av1")]
#[cfg(not(target_arch = "wasm32"))]
pub mod av1;

use crate::Time;

/// One chunk of encoded video data; usually one frame.
pub struct Chunk {
    pub data: Vec<u8>,
    pub timestamp: Time,
    pub duration: Time,
}

/// One decoded video frame.
pub struct Frame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub timestamp: Time,
    pub duration: Time,
}

pub enum PixelFormat {
    Rgba8Unorm,
}
