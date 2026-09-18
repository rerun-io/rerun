use std::fmt::Display;

use super::ValueRange;
use crate::encodings;

impl ValueRange {
    /// Create a new range.
    #[inline]
    pub fn new(start: f64, end: f64) -> Self {
        Self(encodings::Range1D([start, end]))
    }

    /// Converts to finite, distinct `f32` endpoints, preserving their order.
    ///
    /// Returns `None` if conversion overflows or rounds the endpoints to the same value.
    /// Reversed ranges are allowed for inverted color mappings.
    pub fn try_as_f32_range(&self) -> Option<[f32; 2]> {
        let [start, end] = self.0.0.map(|value| value as f32);
        (start.is_finite() && end.is_finite() && start != end).then_some([start, end])
    }

    /// The start of the range.
    #[inline]
    pub fn start(&self) -> f64 {
        self.0.0[0]
    }

    /// The end of the range.
    #[inline]
    pub fn end(&self) -> f64 {
        self.0.0[1]
    }

    /// The start of the range.
    #[inline]
    pub fn start_mut(&mut self) -> &mut f64 {
        &mut self.0.0[0]
    }

    /// The end of the range.
    #[inline]
    pub fn end_mut(&mut self) -> &mut f64 {
        &mut self.0.0[1]
    }
}

impl Display for ValueRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}, {}]", self.start(), self.end())
    }
}

impl Default for ValueRange {
    #[inline]
    fn default() -> Self {
        Self::new(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::ValueRange;

    #[test]
    fn f32_range_preserves_endpoint_order() {
        for endpoints in [[0.0, 1.0], [1.0, 0.0], [-2.0, 3.0]] {
            assert_eq!(
                ValueRange::new(endpoints[0], endpoints[1]).try_as_f32_range(),
                Some(endpoints.map(|value| value as f32)),
            );
        }
    }

    #[test]
    fn f32_range_rejects_non_finite_or_collapsed_endpoints() {
        for [start, end] in [
            [0.0, 0.0],
            [1.0, 1.0 + 1e-10],
            [1.0 + 1e-10, 1.0],
            [0.0, f64::MAX],
            [-f64::MAX, 0.0],
            [f64::NEG_INFINITY, 1.0],
            [0.0, f64::INFINITY],
            [f64::NAN, 1.0],
            [0.0, f64::NAN],
        ] {
            assert_eq!(ValueRange::new(start, end).try_as_f32_range(), None);
        }
    }
}
