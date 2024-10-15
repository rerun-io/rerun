//! Video decoding library.

mod time;

pub mod decode;
pub mod demux;

pub use re_mp4::{TrackId, TrackKind};

pub use self::{
    decode::{Chunk, Frame, PixelFormat},
    demux::{Config, Sample, VideoData, VideoLoadError},
    time::{Time, Timescale},
};
