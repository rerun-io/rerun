use crate::datatypes;
use std::fmt::Display;

use super::Range1D;

impl Range1D {
    /// Create a new range.
    #[inline]
    pub fn new(start: f64, end: f64) -> Self {
        Self(datatypes::Range1D([start, end]))
    }

    /// The start of the range.
    #[inline]
    pub fn start(&self) -> f64 {
        self.0 .0[0]
    }

    /// The end of the range.
    #[inline]
    pub fn end(&self) -> f64 {
        self.0 .0[1]
    }

    /// The start of the range.
    #[inline]
    pub fn start_mut(&mut self) -> &mut f64 {
        &mut self.0 .0[0]
    }

    /// The end of the range.
    #[inline]
    pub fn end_mut(&mut self) -> &mut f64 {
        &mut self.0 .0[1]
    }
}

impl Display for Range1D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}, {}]", self.start(), self.end(),)
    }
}

impl Default for Range1D {
    #[inline]
    fn default() -> Self {
        Self::new(0.0, 1.0)
    }
}
