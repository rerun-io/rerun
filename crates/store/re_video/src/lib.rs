//! Video decoding library.

mod decode;
mod demux;

pub use decode::{av1, Chunk, Frame, PixelFormat};
pub use demux::{Sample, VideoData, VideoLoadError};
pub use re_mp4::{TrackId, TrackKind};

use ordered_float::OrderedFloat;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeMs(OrderedFloat<f64>);

impl TimeMs {
    #[inline]
    pub fn new(v: f64) -> Self {
        Self(OrderedFloat(v))
    }

    #[inline]
    pub fn as_f64(&self) -> f64 {
        self.0.into_inner()
    }
}

/// A value in time units.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Time(u64); // TODO: i64

impl Time {
    pub const ZERO: Self = Self(0);
    pub const MAX: Self = Self(u64::MAX);

    /// Create a new value in _time units_.
    ///
    /// ⚠️ Don't use this for regular timestamps in seconds/milliseconds/etc.,
    /// use the proper constructors for those instead!
    /// This only exists for cases where you already have a value expressed in time units,
    /// such as those received from the `WebCodecs` APIs.
    #[inline]
    pub fn new(v: u64) -> Self {
        Self(v)
    }

    #[inline]
    pub fn from_secs(v: f64, timescale: Timescale) -> Self {
        Self((v * timescale.0 as f64).round() as u64)
    }

    #[inline]
    pub fn from_millis(v: f64, timescale: Timescale) -> Self {
        Self::from_secs(v / 1e3, timescale)
    }

    #[inline]
    pub fn from_micros(v: f64, timescale: Timescale) -> Self {
        Self::from_secs(v / 1e6, timescale)
    }

    #[inline]
    pub fn from_nanos(v: i64, timescale: Timescale) -> Self {
        Self::from_secs(v as f64 / 1e9, timescale)
    }

    #[inline]
    pub fn into_secs(self, timescale: Timescale) -> f64 {
        self.0 as f64 / timescale.0 as f64
    }

    #[inline]
    pub fn into_millis(self, timescale: Timescale) -> f64 {
        self.into_secs(timescale) * 1e3
    }

    #[inline]
    pub fn into_micros(self, timescale: Timescale) -> f64 {
        self.into_secs(timescale) * 1e6
    }

    #[inline]
    pub fn into_nanos(self, timescale: Timescale) -> i64 {
        (self.into_secs(timescale) * 1e9).round() as i64
    }
}

impl std::ops::Sub for Time {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

/// The number of time units per second.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timescale(u64);

impl Timescale {
    pub(crate) fn new(v: u64) -> Self {
        Self(v)
    }
}
