//! Video frame decoding.

pub mod av1;

use crate::TimeMs;

pub struct Chunk {
    pub data: Vec<u8>,
    pub timestamp: TimeMs,
    pub duration: TimeMs,
}

pub struct Frame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub timestamp: TimeMs,
    pub duration: TimeMs,
}

pub enum PixelFormat {
    Rgba8Unorm,
}
