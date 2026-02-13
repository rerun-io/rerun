use re_types_core::datatypes::Float32;

use super::StrokeWidth;

impl Default for StrokeWidth {
    #[inline]
    fn default() -> Self {
        Self(Float32(8.0)) // Reminder: these are ui points. Picking 1.0 is too small, 0.0 is invisible.
    }
}
