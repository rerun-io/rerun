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

/// Which features was this crate compiled with?
pub fn features() -> Vec<&'static str> {
    // TODO(emilk): is there a helper crate for this?
    let mut features = vec![];
    if cfg!(feature = "av1") {
        features.push("av1");
    }
    if cfg!(feature = "nasm") {
        features.push("nasm");
    }
    features
}
