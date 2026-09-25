use std::ops::RangeInclusive;
use std::sync::Arc;

use crate::AudioBuffer;

/// Playback speeds that are heard. Streams at other speeds are silent instead of pitch-shifted,
/// but keep their place in the clip.
pub const AUDIBLE_SPEEDS: RangeInclusive<f32> = 0.25..=4.0;

/// Identifies one playing stream, e.g. a hash of the view and entity it belongs to.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct StreamId(pub u64);

/// What a caller wants to hear right now.
///
/// A stream keeps playing only while every committed update requests it;
/// see `AudioPlayer` for the immediate-mode contract.
pub struct StreamRequest {
    /// Which stream this is.
    ///
    /// Requests with the same id in consecutive updates continue the same stream,
    /// keeping its own cursor unless `position_secs` drifts too far from it.
    pub id: StreamId,

    /// The decoded audio to play.
    pub buffer: Arc<AudioBuffer>,

    /// Where in the buffer the playhead is right now, in seconds.
    pub position_secs: f64,

    /// Playback speed multiplier, where `1.0` is real time.
    /// Anything outside [`AUDIBLE_SPEEDS`] is treated as muted.
    pub speed: f32,

    /// Linear volume multiplier applied to the samples, normally `0..=1`.
    pub volume: f32,
}
