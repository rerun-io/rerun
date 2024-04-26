use crate::datatypes;

use super::Range1D;

impl Range1D {
    #[inline]
    pub fn new(start: f64, end: f64) -> Self {
        Self(datatypes::Range1D([start, end]))
    }

    #[inline]
    pub fn start(&self) -> f64 {
        self.0 .0[0]
    }

    #[inline]
    pub fn end(&self) -> f64 {
        self.0 .0[1]
    }
}
