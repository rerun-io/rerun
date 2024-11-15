//! Video decoding library.

mod time;

pub mod decode;
pub mod demux;

pub use re_mp4::{TrackId, TrackKind};

pub use self::{
    decode::{Chunk, Frame, PixelFormat},
    demux::{Config, Sample, SamplesStatistics, VideoData, VideoLoadError},
    time::{Time, Timescale},
};

/// Returns information about this crate
pub fn build_info() -> re_build_info::BuildInfo {
    re_build_info::build_info!()
}
