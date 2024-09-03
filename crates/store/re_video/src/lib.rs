//! Video decoding library.

pub mod decode;

pub mod demux;

use ordered_float::OrderedFloat;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeMs(OrderedFloat<f64>);

impl TimeMs {
    pub const ZERO: Self = Self(OrderedFloat(0.0));

    pub fn new(ms: f64) -> Self {
        Self(OrderedFloat(ms))
    }

    pub fn as_f64(&self) -> f64 {
        self.0.into_inner()
    }
}

impl std::ops::Add<Self> for TimeMs {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::Sub<Self> for TimeMs {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}
