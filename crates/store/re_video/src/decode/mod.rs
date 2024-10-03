//! Video frame decoding.

#[cfg(feature = "av1")]
pub mod av1;

use crate::Time;

pub struct Chunk {
    pub data: Vec<u8>,
    pub timestamp: Time,
    pub duration: Time,
}

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
