//! Video frame decoding.

pub mod av1;

use crate::TimeMs;

pub struct Chunk {
    pub data: Vec<u8>,
    pub timestamp: TimeMs,
    pub duration: TimeMs,
}

pub struct Frame {}

pub enum PixelFormat {
    NV12,
    YUY2,
    RGB,
}
