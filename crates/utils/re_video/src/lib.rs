//! Video decoding library.

mod time;

pub mod decode;
pub mod demux;
pub mod stable_index_deque;

pub use self::stable_index_deque::StableIndexDeque;
pub use self::{
    decode::{Chunk, Frame, PixelFormat},
    demux::{Sample, SamplesStatistics, VideoCodec, VideoDataDescription, VideoLoadError},
    time::{Time, Timescale},
};

pub use re_mp4::{TrackId, TrackKind};

/// Returns information about this crate
pub fn build_info() -> re_build_info::BuildInfo {
    re_build_info::build_info!()
}
